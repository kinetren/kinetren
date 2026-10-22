#!/usr/bin/env bash
# Generation of test fixtures (conformance test 10, golden test 11).
# Deterministic: fixed ffmpeg generators, no time/randomness.
# The write is atomic (tmp + mv) — a parallel reader will not see an unfinished file.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p out

gen() { # gen <target> <ffmpeg args...>
    local target="$1"; shift
    [ -f "out/$target" ] && return 0
    ffmpeg -v error -y "$@" "out/.tmp.$target"
    mv "out/.tmp.$target" "out/$target"
}

# 2 s of a 440 Hz sine, 44.1k mono AAC (a check of resampling and the lossy anchor)
gen sine44k_mono.m4a -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=2" \
    -c:a aac -b:a 128k -f mp4

# 2 s of a 440 Hz sine, 48k stereo FLAC (lossless: sample accuracy is checked exactly)
gen sine48k_stereo.flac -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" \
    -af "pan=stereo|c0=c0|c1=c0" -c:a flac -f flac

# 2 s testsrc 30 fps H.264 (MT decoder, backward seek)
gen testsrc_h264.mp4 -f lavfi -i "testsrc=size=320x180:rate=30:duration=2" \
    -c:v libx264 -preset veryfast -g 15 -pix_fmt yuv420p -f mp4

# 2 s testsrc 25 fps H.264 (item 14 round 14: the fps of the source ≠ the fps of the project —
# only on such a fixture is the difference between floor/ceil in frame sampling visible;
# at 30 fps both branches degenerate into the same frame)
gen testsrc_25fps.mp4 -f lavfi -i "testsrc=size=320x180:rate=25:duration=2" \
    -c:v libx264 -preset veryfast -g 13 -pix_fmt yuv420p -f mp4

# 2 s testsrc VP9 (threads=1 path)
gen testsrc_vp9.webm -f lavfi -i "testsrc=size=320x180:rate=30:duration=2" \
    -c:v libvpx-vp9 -cpu-used 8 -g 15 -f webm

# R.67 (t796): a SINGLE PICTURE — JPEG and PNG with alpha. Stills were not
# covered by any fixture: “static pictures are not displayed at all, there is
# an eternal Loading...” (the owner). Deterministic lavfi generators.
gen still.jpg -f lavfi -i "testsrc=size=320x180:rate=1:duration=1" -frames:v 1 -q:v 3 -f image2
gen still_alpha.png -f lavfi -i "color=c=red@0.5:size=320x180:rate=1:duration=1,format=rgba" \
    -frames:v 1 -f image2

# R.62 (t711): the SAME pixels 640×480, a DIFFERENT SAR (1:1 and 2:1) — a 4:3 view
# against 8:3; decoded frames at equal Quality must match byte for byte,
# the difference lives only in MediaInfo.sar. One encoder pass for both
# files is impossible (SAR is written into the SPS), therefore — identical deterministic
# generators and identical x264 parameters.
gen testsrc_sar11.mp4 -f lavfi -i "testsrc=size=640x480:rate=25:duration=1" \
    -vf "setsar=1/1" -c:v libx264 -preset veryfast -g 25 -pix_fmt yuv420p -f mp4
gen testsrc_sar21.mp4 -f lavfi -i "testsrc=size=640x480:rate=25:duration=1" \
    -vf "setsar=2/1" -c:v libx264 -preset veryfast -g 25 -pix_fmt yuv420p -f mp4

# 2 s muxed A/V (h264 + aac 48k stereo)
gen muxed_av.mp4 \
    -f lavfi -i "testsrc=size=320x180:rate=30:duration=2" \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" \
    -af "pan=stereo|c0=c0|c1=c0" \
    -c:v libx264 -preset veryfast -g 15 -pix_fmt yuv420p -c:a aac -shortest -f mp4

# 2 s 330 Hz sine ogg/vorbis + COVER ART through METADATA_BLOCK_PICTURE
# (as in real files). A regression of item 26: libavformat exposes the cover
# as a separate attached_pic stream; a foreign stream_index on the lazy
# ogg demuxer hit av_assert0 (stream_index < ogg->nstreams) → abort.
if [ ! -f out/cover_art.ogg ]; then
    ffmpeg -v error -y -f lavfi -i "testsrc=size=64x64:rate=1:duration=1" \
        -frames:v 1 -q:v 5 out/.tmp.cover.jpg
    ffmpeg -v error -y -f lavfi -i "sine=frequency=330:sample_rate=48000:duration=2" \
        -c:a libvorbis -f ogg out/.tmp.plain.ogg
    python3 - <<'PY'
import base64, struct
jpg = open('out/.tmp.cover.jpg','rb').read()
mime = b'image/jpeg'
# FLAC METADATA_BLOCK_PICTURE: type=3 (front cover), BE fields
blk = struct.pack('>I', 3)
blk += struct.pack('>I', len(mime)) + mime
blk += struct.pack('>I', 0)                     # the description is empty
blk += struct.pack('>IIII', 64, 64, 24, 0)      # w, h, depth, colors
blk += struct.pack('>I', len(jpg)) + jpg
open('out/.tmp.mbp.txt','w').write(base64.b64encode(blk).decode())
PY
    ffmpeg -v error -y -i out/.tmp.plain.ogg -c copy \
        -metadata METADATA_BLOCK_PICTURE="$(cat out/.tmp.mbp.txt)" \
        -f ogg out/.tmp.cover_art.ogg
    rm -f out/.tmp.cover.jpg out/.tmp.plain.ogg out/.tmp.mbp.txt
    mv out/.tmp.cover_art.ogg out/cover_art.ogg
fi

echo ok
