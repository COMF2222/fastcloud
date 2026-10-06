# AAC transition regression fixture

Original synthetic audio: 16 seconds of a 440 Hz tone in the right channel;
the left channel is silent. No music, account data or network credentials.
The generated audio is dedicated to the public domain (CC0).

Generated with FFmpeg as AAC stereo at 44.1 kHz, split into four-second fMP4
HLS fragments. Tests include the files directly and need no FFmpeg installation.
Run from this directory to regenerate:

```sh
ffmpeg -f lavfi -i 'aevalsrc=0|0.25*sin(2*PI*440*t):s=44100:d=16' \
  -c:a aac -b:a 48k -f hls -hls_time 4 -hls_playlist_type vod \
  -hls_segment_type fmp4 -hls_fmp4_init_filename init.mp4 \
  -hls_segment_filename 'segment-%d.m4s' index.m3u8
```

The local fake broker advertises the normal AAC stream field, while the lower
encoding bitrate keeps this committed fixture small. The test must decode
through multiple fragments to prepare an eight-second crossfade and then keep
decoding after the next track starts.
