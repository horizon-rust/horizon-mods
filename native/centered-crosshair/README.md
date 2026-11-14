# Centered Crosshair

centered-crosshair, ported 1:1 to Horizon from JustAlittleWolf/centered-crosshair (MIT (c) 2023 JustAlittleWolf, https://github.com/JustAlittleWolf/centered-crosshair).  Upstream is entirely render-side and tiny (4 Java files, 176 LOC). Vanilla draws the crosshair with integer blit coordinates, `x = (w - 15) / 2` in gui-scaled pixels, where `w` is an integer that already lost its fraction to `floor(windowWidth / guiScale)`. When the gui-scaled size is even the sprite lands half a pixel off the true window centre. This port keeps the sprite centred at the real centre, computed from the raw window size, quantised to quarter pixels, and drawn through a subpixel textured-quad path with the sprite's u/v nudged by a 1/32768 half-texel so linear sampling reads the sprite edges cleanly.  Upstream structure mapped 1:1:

Port by sparticle. Upstream: https://github.com/JustAlittleWolf/centered-crosshair (MIT).
