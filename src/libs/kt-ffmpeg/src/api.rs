//! Таблица функций FFmpeg, резолвится в рантайме через dlsym.
//!
//! Ключевой момент архитектуры: бинарь НЕ линкуется с libav* (нет NEEDED),
//! поэтому один и тот же kt-app работает на системе с FFmpeg 6, 7 или 8 —
//! берётся то, что установлено, со всеми его аппаратными ускорениями.
//!
//! Макрос инстанцируется отдельно в каждом версионном модуле (src/v6.rs и
//! т.д.), поэтому сигнатуры ссылаются на типы ИМЕННО той версии — layout
//! структур совпадает с загруженной библиотекой.

/// Объявляет `Api` со списком функций и `Api::load(&Libs)`.
#[macro_export]
macro_rules! declare_api {
    ($( $lib:ident :: $name:ident : $ty:ty ),* $(,)?) => {
        #[allow(non_snake_case)]
        pub struct Api {
            $( pub $name: $ty, )*
        }

        impl Api {
            /// # Safety
            /// Библиотеки должны быть той же мажорной версии, что и типы
            /// этого модуля (гарантирует loader по avcodec_version()).
            pub unsafe fn load(libs: &$crate::loader::Libs) -> Result<Api, $crate::FfError> {
                unsafe {
                    Ok(Api {
                        $( $name: libs.$lib.sym(concat!(stringify!($name), "\0"))?, )*
                    })
                }
            }
        }
    };
}

/// Список функций, общий для всех версий (сигнатуры совпадают в 6/7/8;
/// различающиеся типы приходят из версионного модуля).
#[macro_export]
macro_rules! ffmpeg_api {
    () => {
        use core::ffi::{c_char, c_int, c_uint, c_void};

        $crate::declare_api! {
            // --- avutil ---
            avutil::av_frame_alloc: unsafe extern "C" fn() -> *mut ffi::AVFrame,
            avutil::av_frame_free: unsafe extern "C" fn(*mut *mut ffi::AVFrame),
            avutil::av_frame_unref: unsafe extern "C" fn(*mut ffi::AVFrame),
            avutil::av_frame_get_buffer: unsafe extern "C" fn(*mut ffi::AVFrame, c_int) -> c_int,
            avutil::av_rescale_q: unsafe extern "C" fn(i64, ffi::AVRational, ffi::AVRational) -> i64,
            avutil::av_channel_layout_default: unsafe extern "C" fn(*mut ffi::AVChannelLayout, c_int),
            avutil::av_channel_layout_copy:
                unsafe extern "C" fn(*mut ffi::AVChannelLayout, *const ffi::AVChannelLayout) -> c_int,
            avutil::av_channel_layout_uninit: unsafe extern "C" fn(*mut ffi::AVChannelLayout),
            avutil::av_dict_set:
                unsafe extern "C" fn(*mut *mut ffi::AVDictionary, *const c_char, *const c_char, c_int) -> c_int,
            avutil::av_dict_free: unsafe extern "C" fn(*mut *mut ffi::AVDictionary),
            avutil::av_log_set_level: unsafe extern "C" fn(c_int),
            avutil::av_opt_set:
                unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, c_int) -> c_int,
            avutil::av_strerror: unsafe extern "C" fn(c_int, *mut c_char, usize) -> c_int,
            // Р.67: у формата пикселей спрашивают, есть ли АЛЬФА — иначе
            // прозрачность PNG/ProRes 4444 выбрасывалась молча, и логотип
            // ложился на видео непрозрачным прямоугольником (F-MED-003)
            avutil::av_pix_fmt_desc_get:
                unsafe extern "C" fn(ffi::AVPixelFormat) -> *const ffi::AVPixFmtDescriptor,

            // --- avcodec ---
            avcodec::avcodec_version: unsafe extern "C" fn() -> c_uint,
            avcodec::avcodec_find_decoder: unsafe extern "C" fn(ffi::AVCodecID) -> *const ffi::AVCodec,
            avcodec::avcodec_find_encoder_by_name: unsafe extern "C" fn(*const c_char) -> *const ffi::AVCodec,
            avcodec::avcodec_alloc_context3: unsafe extern "C" fn(*const ffi::AVCodec) -> *mut ffi::AVCodecContext,
            avcodec::avcodec_free_context: unsafe extern "C" fn(*mut *mut ffi::AVCodecContext),
            avcodec::avcodec_parameters_to_context:
                unsafe extern "C" fn(*mut ffi::AVCodecContext, *const ffi::AVCodecParameters) -> c_int,
            avcodec::avcodec_parameters_from_context:
                unsafe extern "C" fn(*mut ffi::AVCodecParameters, *const ffi::AVCodecContext) -> c_int,
            avcodec::avcodec_open2:
                unsafe extern "C" fn(*mut ffi::AVCodecContext, *const ffi::AVCodec, *mut *mut ffi::AVDictionary) -> c_int,
            avcodec::avcodec_send_packet:
                unsafe extern "C" fn(*mut ffi::AVCodecContext, *const ffi::AVPacket) -> c_int,
            avcodec::avcodec_receive_frame:
                unsafe extern "C" fn(*mut ffi::AVCodecContext, *mut ffi::AVFrame) -> c_int,
            avcodec::avcodec_send_frame:
                unsafe extern "C" fn(*mut ffi::AVCodecContext, *const ffi::AVFrame) -> c_int,
            avcodec::avcodec_receive_packet:
                unsafe extern "C" fn(*mut ffi::AVCodecContext, *mut ffi::AVPacket) -> c_int,
            avcodec::avcodec_flush_buffers: unsafe extern "C" fn(*mut ffi::AVCodecContext),
            avcodec::av_packet_alloc: unsafe extern "C" fn() -> *mut ffi::AVPacket,
            avcodec::av_packet_free: unsafe extern "C" fn(*mut *mut ffi::AVPacket),
            avcodec::av_packet_unref: unsafe extern "C" fn(*mut ffi::AVPacket),
            avcodec::av_packet_rescale_ts:
                unsafe extern "C" fn(*mut ffi::AVPacket, ffi::AVRational, ffi::AVRational),

            // --- avformat ---
            avformat::avformat_open_input: unsafe extern "C" fn(
                *mut *mut ffi::AVFormatContext, *const c_char,
                *const ffi::AVInputFormat, *mut *mut ffi::AVDictionary) -> c_int,
            avformat::avformat_close_input: unsafe extern "C" fn(*mut *mut ffi::AVFormatContext),
            avformat::avformat_find_stream_info:
                unsafe extern "C" fn(*mut ffi::AVFormatContext, *mut *mut ffi::AVDictionary) -> c_int,
            avformat::av_find_best_stream: unsafe extern "C" fn(
                *mut ffi::AVFormatContext, ffi::AVMediaType, c_int, c_int,
                *mut *const ffi::AVCodec, c_int) -> c_int,
            avformat::av_read_frame: unsafe extern "C" fn(*mut ffi::AVFormatContext, *mut ffi::AVPacket) -> c_int,
            avformat::av_seek_frame:
                unsafe extern "C" fn(*mut ffi::AVFormatContext, c_int, i64, c_int) -> c_int,
            avformat::avformat_seek_file:
                unsafe extern "C" fn(*mut ffi::AVFormatContext, c_int, i64, i64, i64, c_int) -> c_int,
            avformat::avformat_alloc_output_context2: unsafe extern "C" fn(
                *mut *mut ffi::AVFormatContext, *const ffi::AVOutputFormat,
                *const c_char, *const c_char) -> c_int,
            avformat::avformat_free_context: unsafe extern "C" fn(*mut ffi::AVFormatContext),
            avformat::avformat_new_stream:
                unsafe extern "C" fn(*mut ffi::AVFormatContext, *const ffi::AVCodec) -> *mut ffi::AVStream,
            avformat::avformat_write_header:
                unsafe extern "C" fn(*mut ffi::AVFormatContext, *mut *mut ffi::AVDictionary) -> c_int,
            avformat::av_interleaved_write_frame:
                unsafe extern "C" fn(*mut ffi::AVFormatContext, *mut ffi::AVPacket) -> c_int,
            avformat::av_write_trailer: unsafe extern "C" fn(*mut ffi::AVFormatContext) -> c_int,
            avformat::avio_open: unsafe extern "C" fn(*mut *mut ffi::AVIOContext, *const c_char, c_int) -> c_int,
            avformat::avio_closep: unsafe extern "C" fn(*mut *mut ffi::AVIOContext) -> c_int,

            // --- swscale ---
            swscale::sws_getContext: unsafe extern "C" fn(
                c_int, c_int, ffi::AVPixelFormat, c_int, c_int, ffi::AVPixelFormat,
                c_int, *mut c_void, *mut c_void, *const f64) -> *mut ffi::SwsContext,
            swscale::sws_freeContext: unsafe extern "C" fn(*mut ffi::SwsContext),
            swscale::sws_scale: unsafe extern "C" fn(
                *mut ffi::SwsContext, *const *const u8, *const c_int, c_int, c_int,
                *const *mut u8, *const c_int) -> c_int,

            // --- swresample ---
            swresample::swr_alloc_set_opts2: unsafe extern "C" fn(
                *mut *mut ffi::SwrContext,
                *const ffi::AVChannelLayout, ffi::AVSampleFormat, c_int,
                *const ffi::AVChannelLayout, ffi::AVSampleFormat, c_int,
                c_int, *mut c_void) -> c_int,
            swresample::swr_init: unsafe extern "C" fn(*mut ffi::SwrContext) -> c_int,
            swresample::swr_free: unsafe extern "C" fn(*mut *mut ffi::SwrContext),
            swresample::swr_convert: unsafe extern "C" fn(
                *mut ffi::SwrContext, *mut *mut u8, c_int, *mut *const u8, c_int) -> c_int,
            swresample::swr_get_out_samples: unsafe extern "C" fn(*mut ffi::SwrContext, c_int) -> i64,
        }
    };
}
