# Generated Vorbis fixtures

These are synthetic 37 ms sine waves encoded with FFmpeg's libvorbis encoder.
Tests concatenate complete Ogg files to exercise logical-link boundaries.
No external encoder is required to run the tests.

Regenerate each file with this command, substituting the table values:

\```sh
ffmpeg -v error -f lavfi -i "sine=frequency=$frequency:sample_rate=$rate:duration=0.037" \
  -ac "$channels" -c:a libvorbis -flags:a +bitexact -fflags +bitexact \
  -serial_offset "$serial" -map_metadata -1 -y "$file"
\```

| File | Frequency | Rate | Channels | Serial |
| --- | ---: | ---: | ---: | ---: |
| mono-44100.ogg | 440 | 44100 | 1 | 1 |
| stereo-44100.ogg | 880 | 44100 | 2 | 2 |
| mono-48000.ogg | 660 | 48000 | 1 | 3 |
