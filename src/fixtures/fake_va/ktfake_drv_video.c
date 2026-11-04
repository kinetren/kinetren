// A VA-API driver without a GPU (hw-encode-context): loaded by the REAL libva into the REAL FFmpeg,
// so the hardware path of the export runs on a machine with no graphics card, CI included. Each
// way a driver is known to fail is one value of KT_FAKE_VA; the driver is what t1551–t1556 test.
//
// With no failure asked for it is an H.264 encoder that keeps every sample: FFmpeg writes the
// headers (packed), this file writes the slice data as I_PCM macroblocks read from the surface.
// So a file it made decodes to exactly what was uploaded — the proof of the upload path, NOT of
// any real driver, and never a measurement of speed or quality.
//
// No libva-dev here: the structures below mirror libva's stable ABI (major 1), and only the fields
// used are named. Built by the test: `cc -shared -fPIC`.
//
//   KT_FAKE_VA          init-abort, init-refuse, surfaces-refuse, context-refuse, context-abort,
//                       context-hang, frame-refuse, frame-abort, frame-refuse-after-N, frame-empty
//   KT_FAKE_VA_LOG      a file: one line per process that finished, "frames N"
//
// The same library is LD_PRELOADed for `drmGetNodeTypeFromFd`: libva refuses a file that is not a
// DRM node, and a CI runner has none. The test's "render node" is a regular file.

#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

enum {
    VA_OK = 0,
    VA_ERR_OPERATION_FAILED = 1,
    VA_ERR_ALLOCATION_FAILED = 2,
    VA_ERR_INVALID_BUFFER = 7,
    VA_ERR_UNIMPLEMENTED = 0x14,
};

#define VA_ATTRIB_NOT_SUPPORTED 0x80000000u
#define FOURCC_NV12 0x3231564eu
#define PROFILE_H264_CB 13
#define ENTRYPOINT_ENC_SLICE 6

typedef struct {
    int type;
    uint32_t value;
} ConfigAttrib;

typedef struct {
    uint32_t fourcc, byte_order, bits_per_pixel, depth, red, green, blue, alpha;
    uint32_t reserved[4];
} ImageFormat;

typedef struct {
    uint32_t image_id;
    ImageFormat format;
    uint32_t buf;
    uint16_t width, height;
    uint32_t data_size, num_planes;
    uint32_t pitches[3], offsets[3];
    int32_t num_palette_entries, entry_bytes;
    int8_t component_order[4];
    uint32_t reserved[4];
} Image;

typedef struct {
    int type;
    uint32_t flags;
    struct {
        int type;
        union {
            int i;
            void *p;
        } value;
    } value;
} SurfaceAttrib;

typedef struct {
    uint32_t size, bit_offset, status, reserved;
    void *buf, *next;
    uint32_t reserved2[4];
} CodedSegment;

typedef int (*Fn)();

enum {
    V_Terminate, V_QueryConfigProfiles, V_QueryConfigEntrypoints, V_GetConfigAttributes,
    V_CreateConfig, V_DestroyConfig, V_QueryConfigAttributes, V_CreateSurfaces, V_DestroySurfaces,
    V_CreateContext, V_DestroyContext, V_CreateBuffer, V_BufferSetNumElements, V_MapBuffer,
    V_UnmapBuffer, V_DestroyBuffer, V_BeginPicture, V_RenderPicture, V_EndPicture, V_SyncSurface,
    V_QuerySurfaceStatus, V_QuerySurfaceError, V_PutSurface, V_QueryImageFormats, V_CreateImage,
    V_DeriveImage, V_DestroyImage, V_SetImagePalette, V_GetImage, V_PutImage,
    V_QuerySubpictureFormats, V_CreateSubpicture, V_DestroySubpicture, V_SetSubpictureImage,
    V_SetSubpictureChromakey, V_SetSubpictureGlobalAlpha, V_AssociateSubpicture,
    V_DeassociateSubpicture, V_QueryDisplayAttributes, V_GetDisplayAttributes,
    V_SetDisplayAttributes, V_BufferInfo, V_LockSurface, V_UnlockSurface, V_GetSurfaceAttributes,
    V_CreateSurfaces2, V_QuerySurfaceAttributes, V_COUNT,
};

// the head of VADriverContext; libva owns the structure
typedef struct {
    void *driver_data;
    Fn *vtable;
    void *vtable_glx, *vtable_egl, *vtable_tpi, *native_dpy;
    int x11_screen, version_major, version_minor;
    int max_profiles, max_entrypoints, max_attributes, max_image_formats, max_subpic_formats,
        max_display_attributes;
    const char *str_vendor;
} Ctx;

#define MAX_SURFACES 512
#define MAX_BUFFERS 8192
#define MAX_PACKED 16

typedef struct {
    int used;
    unsigned w, h;
    uint8_t *mem;
} Surface;

typedef struct {
    int used, type, borrowed;
    unsigned size;
    uint8_t *data;
    CodedSegment seg;
} Buffer;

static Surface surfaces[MAX_SURFACES];
static Buffer buffers[MAX_BUFFERS];
static uint32_t next_image = 1;
static unsigned coded_w, coded_h;
static uint32_t target;
static uint32_t coded_buf;
static int slice_type;
static long frames;
// the packed headers of the picture, in the order they came
static struct {
    uint32_t type, bits;
    uint8_t *data;
    unsigned bytes;
} packed[MAX_PACKED];
static int packed_n;
static uint32_t pending_type, pending_bits;

static const char *mode(void) {
    const char *m = getenv("KT_FAKE_VA");
    return m ? m : "";
}

static int is_mode(const char *m) {
    return strcmp(mode(), m) == 0;
}

static int unimplemented(void) {
    return VA_ERR_UNIMPLEMENTED;
}

static int ok(void) {
    return VA_OK;
}

// ---- the bitstream: start with FFmpeg's slice header, go on bit by bit -------------------------

typedef struct {
    uint8_t *out;
    unsigned pos, room;
    uint32_t acc;
    int nbits, zeros;
} Bits;

static void put_byte(Bits *b, uint8_t v) {
    // emulation prevention: the header bytes already carry theirs, ours are counted from there
    if (b->zeros >= 2 && v <= 3) {
        if (b->pos < b->room) b->out[b->pos++] = 3;
        b->zeros = 0;
    }
    if (b->pos < b->room) b->out[b->pos++] = v;
    b->zeros = v == 0 ? b->zeros + 1 : 0;
}

static void put_bits(Bits *b, uint32_t v, int n) {
    while (n-- > 0) {
        b->acc = (b->acc << 1) | ((v >> n) & 1);
        if (++b->nbits == 8) {
            put_byte(b, (uint8_t)b->acc);
            b->acc = 0;
            b->nbits = 0;
        }
    }
}

static void put_ue(Bits *b, uint32_t v) {
    uint32_t code = v + 1;
    int len = 0;
    while ((code >> (len + 1)) != 0) len++;
    put_bits(b, 0, len);
    put_bits(b, code, len + 1);
}

static void align(Bits *b) {
    if (b->nbits) put_bits(b, 0, 8 - b->nbits);
}

static void write_macroblock(Bits *b, const Surface *s, unsigned mbx, unsigned mby) {
    unsigned x, y;
    if (slice_type != 2) put_ue(b, 0); // mb_skip_run
    put_ue(b, slice_type == 2 ? 25 : 30); // mb_type: I_PCM
    align(b);
    for (y = 0; y < 16; y++) {
        unsigned sy = mby * 16 + y;
        if (sy >= s->h) sy = s->h - 1;
        for (x = 0; x < 16; x++) {
            unsigned sx = mbx * 16 + x;
            if (sx >= s->w) sx = s->w - 1;
            put_byte(b, s->mem[sy * s->w + sx]);
        }
    }
    for (int plane = 0; plane < 2; plane++) {
        for (y = 0; y < 8; y++) {
            unsigned sy = mby * 8 + y;
            if (sy >= s->h / 2) sy = s->h / 2 - 1;
            for (x = 0; x < 8; x++) {
                unsigned sx = mbx * 8 + x;
                if (sx >= s->w / 2) sx = s->w / 2 - 1;
                put_byte(b, s->mem[s->w * s->h + sy * s->w + sx * 2 + plane]);
            }
        }
    }
}

// ---- the entry points --------------------------------------------------------------------------

static int terminate(void *ctx) {
    const char *log = getenv("KT_FAKE_VA_LOG");
    (void)ctx;
    if (log && frames > 0) {
        FILE *f = fopen(log, "a");
        if (f) {
            fprintf(f, "frames %ld\n", frames);
            fclose(f);
        }
    }
    return VA_OK;
}

static int query_profiles(void *ctx, int *list, int *n) {
    (void)ctx;
    list[0] = PROFILE_H264_CB;
    *n = 1;
    return VA_OK;
}

static int query_entrypoints(void *ctx, int profile, int *list, int *n) {
    (void)ctx, (void)profile;
    list[0] = ENTRYPOINT_ENC_SLICE;
    *n = 1;
    return VA_OK;
}

static int get_attributes(void *ctx, int profile, int ep, ConfigAttrib *a, int n) {
    (void)ctx, (void)profile, (void)ep;
    for (int i = 0; i < n; i++) {
        switch (a[i].type) {
        case 0: a[i].value = 1; break;      // RT format: YUV420
        case 5: a[i].value = 0x10; break;   // rate control: CQP
        case 10: a[i].value = 0xd; break;   // packed: sequence, slice, misc
        case 13: a[i].value = 1; break;     // one reference, no B
        default: a[i].value = VA_ATTRIB_NOT_SUPPORTED;
        }
    }
    return VA_OK;
}

static int create_config(void *ctx, int p, int e, void *a, int n, uint32_t *id) {
    (void)ctx, (void)p, (void)e, (void)a, (void)n;
    *id = 1;
    return VA_OK;
}

static int create_surfaces2(void *ctx, unsigned format, unsigned w, unsigned h, uint32_t *out,
                            unsigned n, void *attr, unsigned nattr) {
    (void)ctx, (void)format, (void)attr, (void)nattr;
    if (is_mode("surfaces-refuse")) return VA_ERR_ALLOCATION_FAILED;
    for (unsigned i = 0; i < n; i++) {
        unsigned id = 0;
        for (unsigned s = 1; s < MAX_SURFACES; s++) {
            if (!surfaces[s].used) {
                id = s;
                break;
            }
        }
        if (!id) return VA_ERR_ALLOCATION_FAILED;
        surfaces[id].used = 1;
        surfaces[id].w = w;
        surfaces[id].h = h;
        surfaces[id].mem = calloc(1, w * h * 3 / 2);
        out[i] = id;
    }
    return VA_OK;
}

static int create_surfaces(void *ctx, int w, int h, int format, int n, uint32_t *out) {
    return create_surfaces2(ctx, format, w, h, out, n, NULL, 0);
}

static int destroy_surfaces(void *ctx, uint32_t *list, int n) {
    (void)ctx;
    for (int i = 0; i < n; i++) {
        free(surfaces[list[i]].mem);
        surfaces[list[i]].mem = NULL;
        surfaces[list[i]].used = 0;
    }
    return VA_OK;
}

static int create_context(void *ctx, uint32_t config, int w, int h, int flag, uint32_t *targets,
                          int n, uint32_t *id) {
    (void)ctx, (void)config, (void)flag, (void)targets, (void)n;
    if (is_mode("context-refuse")) return VA_ERR_OPERATION_FAILED;
    if (is_mode("context-abort")) abort();
    if (is_mode("context-hang")) {
        for (;;) sleep(1000);
    }
    coded_w = w;
    coded_h = h;
    *id = 1;
    return VA_OK;
}

static int new_buffer(int type, unsigned size, uint32_t *id) {
    for (unsigned b = 1; b < MAX_BUFFERS; b++) {
        if (!buffers[b].used) {
            memset(&buffers[b], 0, sizeof buffers[b]);
            buffers[b].used = 1;
            buffers[b].type = type;
            buffers[b].size = size;
            *id = b;
            return VA_OK;
        }
    }
    return VA_ERR_ALLOCATION_FAILED;
}

static int create_buffer(void *ctx, uint32_t context, int type, unsigned size, unsigned n,
                         void *data, uint32_t *id) {
    (void)ctx, (void)context;
    int r = new_buffer(type, size * n, id);
    if (r != VA_OK) return r;
    buffers[*id].data = malloc(size * n + 1);
    if (data) memcpy(buffers[*id].data, data, size * n);
    return VA_OK;
}

static int map_buffer(void *ctx, uint32_t id, void **out) {
    (void)ctx;
    if (id == 0 || id >= MAX_BUFFERS || !buffers[id].used) return VA_ERR_INVALID_BUFFER;
    *out = buffers[id].type == 21 ? (void *)&buffers[id].seg : (void *)buffers[id].data;
    return VA_OK;
}

static int destroy_buffer(void *ctx, uint32_t id) {
    (void)ctx;
    if (id == 0 || id >= MAX_BUFFERS || !buffers[id].used) return VA_ERR_INVALID_BUFFER;
    if (!buffers[id].borrowed) free(buffers[id].data);
    buffers[id].used = 0;
    return VA_OK;
}

static int begin_picture(void *ctx, uint32_t context, uint32_t surface) {
    (void)ctx, (void)context;
    target = surface;
    packed_n = 0;
    return VA_OK;
}

static int render_picture(void *ctx, uint32_t context, uint32_t *list, int n) {
    (void)ctx, (void)context;
    for (int i = 0; i < n; i++) {
        Buffer *b = &buffers[list[i]];
        switch (b->type) {
        case 23: // VAEncPictureParameterBufferH264: after CurrPic and 16 references (36 bytes each)
            memcpy(&coded_buf, b->data + 17 * 36, 4);
            break;
        case 24: // VAEncSliceParameterBufferH264: slice_type after three 32-bit fields
            slice_type = b->data[12];
            break;
        case 25: // VAEncPackedHeaderParameterBuffer
            memcpy(&pending_type, b->data, 4);
            memcpy(&pending_bits, b->data + 4, 4);
            break;
        case 26: // its data
            if (packed_n < MAX_PACKED) {
                packed[packed_n].type = pending_type;
                packed[packed_n].bits = pending_bits;
                packed[packed_n].bytes = (pending_bits + 7) / 8;
                packed[packed_n].data = malloc(packed[packed_n].bytes);
                memcpy(packed[packed_n].data, b->data, packed[packed_n].bytes);
                packed_n++;
            }
            break;
        default:
            break;
        }
    }
    return VA_OK;
}

static int end_picture(void *ctx, uint32_t context) {
    (void)ctx, (void)context;
    const char *m = mode();
    if (is_mode("frame-refuse")) return VA_ERR_OPERATION_FAILED;
    if (is_mode("frame-abort")) abort();
    if (strncmp(m, "frame-refuse-after-", 19) == 0 && frames >= atol(m + 19))
        return VA_ERR_OPERATION_FAILED;
    frames++;

    Buffer *coded = &buffers[coded_buf];
    if (!coded->used || !coded->data) return VA_ERR_INVALID_BUFFER;
    Bits b = {.out = coded->data, .room = coded->size};
    Surface *s = &surfaces[target];
    for (int i = 0; i < packed_n; i++) {
        unsigned whole = packed[i].bits / 8;
        memcpy(b.out + b.pos, packed[i].data, whole);
        b.pos += whole;
        if (packed[i].type == 3) { // the slice header: its last bits, then the macroblocks
            b.zeros = 0;
            for (unsigned z = b.pos; z > 0 && b.out[z - 1] == 0; z--) b.zeros++;
            int rest = packed[i].bits % 8;
            if (rest) put_bits(&b, packed[i].data[whole] >> (8 - rest), rest);
            if (!is_mode("frame-empty")) {
                for (unsigned y = 0; y < coded_h / 16; y++) {
                    for (unsigned x = 0; x < coded_w / 16; x++) write_macroblock(&b, s, x, y);
                }
            }
            put_bits(&b, 1, 1); // rbsp_stop_one_bit
            align(&b);
        }
        free(packed[i].data);
    }
    packed_n = 0;
    coded->seg.size = is_mode("frame-empty") ? 0 : b.pos;
    coded->seg.buf = coded->data;
    coded->seg.next = NULL;
    return VA_OK;
}

static int query_image_formats(void *ctx, ImageFormat *list, int *n) {
    (void)ctx;
    memset(&list[0], 0, sizeof list[0]);
    list[0].fourcc = FOURCC_NV12;
    list[0].bits_per_pixel = 12;
    *n = 1;
    return VA_OK;
}

static int derive_image(void *ctx, uint32_t surface, Image *img) {
    (void)ctx;
    Surface *s = &surfaces[surface];
    int r = new_buffer(9, s->w * s->h * 3 / 2, &img->buf);
    if (r != VA_OK) return r;
    buffers[img->buf].data = s->mem;
    buffers[img->buf].borrowed = 1;
    memset(&img->format, 0, sizeof img->format);
    img->image_id = next_image++;
    img->format.fourcc = FOURCC_NV12;
    img->format.bits_per_pixel = 12;
    img->width = s->w;
    img->height = s->h;
    img->data_size = s->w * s->h * 3 / 2;
    img->num_planes = 2;
    img->pitches[0] = img->pitches[1] = s->w;
    img->offsets[0] = 0;
    img->offsets[1] = s->w * s->h;
    return VA_OK;
}

static int destroy_image(void *ctx, uint32_t id) {
    (void)ctx, (void)id;
    return VA_OK;
}

static int query_surface_attributes(void *ctx, uint32_t config, SurfaceAttrib *list, unsigned *n) {
    (void)ctx, (void)config;
    if (list) {
        memset(&list[0], 0, sizeof list[0]);
        list[0].type = 1; // pixel format
        list[0].flags = 3;
        list[0].value.type = 1;
        list[0].value.value.i = (int)FOURCC_NV12;
    }
    *n = 1;
    return VA_OK;
}

int __vaDriverInit_1_0(Ctx *ctx) {
    if (is_mode("init-abort")) abort();
    if (is_mode("init-refuse")) return VA_ERR_OPERATION_FAILED;
    for (int i = 0; i < V_COUNT; i++) ctx->vtable[i] = unimplemented;
    ctx->version_major = 1;
    ctx->version_minor = 0;
    ctx->max_profiles = 1;
    ctx->max_entrypoints = 1;
    ctx->max_attributes = 8;
    ctx->max_image_formats = 1;
    ctx->max_subpic_formats = 1;
    ctx->max_display_attributes = 1;
    ctx->str_vendor = "Kinetren fake VA driver";
    ctx->vtable[V_Terminate] = (Fn)terminate;
    ctx->vtable[V_QueryConfigProfiles] = (Fn)query_profiles;
    ctx->vtable[V_QueryConfigEntrypoints] = (Fn)query_entrypoints;
    ctx->vtable[V_GetConfigAttributes] = (Fn)get_attributes;
    ctx->vtable[V_CreateConfig] = (Fn)create_config;
    ctx->vtable[V_DestroyConfig] = ok;
    ctx->vtable[V_CreateSurfaces] = (Fn)create_surfaces;
    ctx->vtable[V_CreateSurfaces2] = (Fn)create_surfaces2;
    ctx->vtable[V_DestroySurfaces] = (Fn)destroy_surfaces;
    ctx->vtable[V_CreateContext] = (Fn)create_context;
    ctx->vtable[V_DestroyContext] = ok;
    ctx->vtable[V_CreateBuffer] = (Fn)create_buffer;
    ctx->vtable[V_MapBuffer] = (Fn)map_buffer;
    ctx->vtable[V_UnmapBuffer] = ok;
    ctx->vtable[V_DestroyBuffer] = (Fn)destroy_buffer;
    ctx->vtable[V_BeginPicture] = (Fn)begin_picture;
    ctx->vtable[V_RenderPicture] = (Fn)render_picture;
    ctx->vtable[V_EndPicture] = (Fn)end_picture;
    ctx->vtable[V_SyncSurface] = ok;
    ctx->vtable[V_QueryImageFormats] = (Fn)query_image_formats;
    ctx->vtable[V_DeriveImage] = (Fn)derive_image;
    ctx->vtable[V_DestroyImage] = (Fn)destroy_image;
    ctx->vtable[V_QuerySurfaceAttributes] = (Fn)query_surface_attributes;
    return VA_OK;
}

// LD_PRELOADed: a regular file passes libva's "is this a DRM node" (DRM_NODE_RENDER)
int drmGetNodeTypeFromFd(int fd) {
    (void)fd;
    return 2;
}
