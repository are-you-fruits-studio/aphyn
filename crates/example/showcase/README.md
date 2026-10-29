# aphyn_example_showcase

A seamless looping scene: three boid flocks (450 entities) steered by ECS systems around a lit berry,
with emissive accents, bloom, exposure and ACES grading, a normal-mapped floor, debug gizmo lines and a
bitmap-font overlay. Every mesh and texture is procedural, so nothing is loaded from disk.

```bash
cargo run --release -p aphyn_example_showcase
```

The simulation advances one fixed 1/60 s step per frame, uses a fixed seed, and the camera orbits once
per 7 s loop. Separation between boids fades out before the loop seam, so the damped seek springs settle
every boid onto the same periodic path each loop and the last frame flows into the first.

## Capturing the loop

```bash
cargo run --release -p aphyn_example_showcase -- --capture target/showcase_frames
```

The run plays 4 warm-up loops, then reads back the engine's render target (the same image the window
shows, without the title bar) and writes the 210 frames of one loop, at 30 fps, as
`target/showcase_frames/frame_0000.png` … `frame_0209.png`, then exits. At every loop boundary it prints
the largest boid position drift since the previous loop; the captured loop should report about `0.000006`
world units, far below a pixel. The window must open at 1280×720 physical pixels, or a warning is printed.

## Converting with ffmpeg

Run from the repository root. The GIF is 800×450, 30 fps, loops forever, with a 40-color generated palette,
which keeps it under 5 MB:

```bash
ffmpeg -y -framerate 30 -i target/showcase_frames/frame_%04d.png -vf "scale=800:450:flags=area,palettegen=max_colors=40:stats_mode=full" target/showcase_palette.png
```

```bash
ffmpeg -y -framerate 30 -i target/showcase_frames/frame_%04d.png -i target/showcase_palette.png -lavfi "scale=800:450:flags=area[x];[x][1:v]paletteuse=dither=none:diff_mode=rectangle" -loop 0 target/showcase.gif
```

The MP4 for social posts keeps the full 1280×720:

```bash
ffmpeg -y -framerate 30 -i target/showcase_frames/frame_%04d.png -c:v libx264 -preset veryfast -crf 18 -pix_fmt yuv420p -movflags +faststart target/showcase.mp4
```

At 960×540 the GIF comes out around 8 MB because the orbiting camera changes every pixel every frame; dithering
also pushes it over 5 MB.
