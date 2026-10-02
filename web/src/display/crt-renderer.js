// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The picture: draws the emulator's raw RGBA raster onto a curved, masked
// piece of picture-tube glass (or a flat panel) with WebGL.
//
// What it models, in order (all in one fragment shader, then a glow and a
// phosphor-persistence pass):
//  - Composite/RF video bandwidth: each scanline is filtered horizontally in
//    YUV, with luma kept fairly sharp and chroma blurred over ~3 pixels, so
//    colour edges bleed the way they do on a real PAL TV/monitor.
//  - The electron beam: every raster line is a Gaussian spot whose height
//    grows with brightness (bright text "blooms", dark areas show the gaps
//    between scanlines -- the look of the Diamond Pro photo).
//  - Tube geometry: barrel curvature, a little overscan, and the glass
//    outline cut from the photo of the 2002 monitor (mask texture).
//  - A shadow mask (RGB slot triads at device-pixel scale, kept subtle).
//  - Halation/glow (blurred copy added back), phosphor persistence, unlit
//    phosphor grey and a soft reflection on the glass.
//  - Power on/off: warm-up fade, and the classic collapse to a line and a
//    fading dot at switch-off.
//  - The monitor's own controls (see monitor-2002.js): horizontal position,
//    vertical hold (the picture rolls when it can't lock), colour, tint,
//    brightness and contrast.
//
// Shaders are WebGL1 / GLSL ES 1.0 so this runs everywhere. If WebGL is
// unavailable it falls back to a plain smoothed 2D blit.

const VS = `
attribute vec2 aPos;
varying vec2 vUV;
void main() {
  vUV = vec2(aPos.x * 0.5 + 0.5, 0.5 - aPos.y * 0.5); // (0,0) = top-left
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;

const FS_CRT = `
precision highp float;
varying vec2 vUV;
uniform sampler2D uSrc;
uniform sampler2D uMask;
uniform vec2 uSrcSize;
uniform vec4 uWin;      // visible raster window in source px: x0, y0, w, h
uniform vec2 uOut;      // output size in device px
uniform float uCurv;    // barrel curvature
uniform float uOverscan;
uniform float uScan;    // scanline strength 0..1
uniform float uMaskAmt; // shadow-mask strength 0..1
uniform float uSharp;   // luma filter: exp(-d^2 * uSharp)
uniform float uChroma;  // chroma filter: exp(-d^2 * uChroma)
uniform float uBright;
uniform vec3 uBeam;     // x: vertical squash, y: horizontal squash, z: beam intensity
// Video connection (signal path between the C64 and the display):
uniform float uTime;    // seconds, for noise/crawl animation
uniform float uFsc;     // colour subcarrier cycles per C64 pixel (PAL 0.5625, NTSC 0.4375)
uniform float uXtalk;   // luma/chroma crosstalk (dot crawl, rainbows): 0 on S-Video
uniform float uNoise;   // random noise ("snow" on RF)
uniform float uGhost;   // multipath/reflection ghost strength
uniform float uGhostOff;// ghost delay in pixels
uniform float uJitter;  // per-line horizontal sync jitter in pixels
uniform float uHum;     // slowly rolling hum bar depth
// Monitor controls:
uniform float uHPos;    // picture shift in source px
uniform float uRoll;    // vertical roll in lines (vertical hold out of lock)
uniform float uFrameH;  // lines per frame
uniform vec2 uBlank;    // lines outside [x, y) are in vertical blanking
uniform float uSat;     // colour (saturation) gain
uniform float uTint;    // hue rotation, radians
uniform float uBlack;   // black level (brightness knob)
uniform float uGain;    // video gain (contrast knob)

const mat3 toYUV = mat3(0.299, -0.14713, 0.615,  0.587, -0.28886, -0.51499,  0.114, 0.436, -0.10001);
const mat3 toRGB = mat3(1.0, 1.0, 1.0,  0.0, -0.39465, 2.03211,  1.13983, -0.58060, 0.0);

float hash(vec3 p) { return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453); }

// One raster line as it arrives at the display: band-limited luma and
// chroma (separately on S-Video, sharing one wire on composite/RF, where
// each leaks into the other), plus noise, ghosting and sync jitter.
vec3 lineColor(float line, float x) {
  line = mod(line, uFrameH);
  if (line < uBlank.x || line >= uBlank.y) return vec3(0.0); // the blanking bar of a rolling picture
  float frame = floor(uTime * 50.0);
  float y = line + 0.5;
  x += uJitter * (hash(vec3(line, frame, 1.7)) - 0.5);
  float fx = x - 0.5;
  float base = floor(fx);
  float parity = mod(frame, 2.0) * 0.5;
  float accY = 0.0, wY = 0.0, wC = 0.0, edge = 0.0;
  vec2 accC = vec2(0.0);
  for (int i = -3; i <= 4; i++) {
    float px = base + float(i);
    float d = px - fx;
    vec3 yuv = toYUV * texture2D(uSrc, vec2(px + 0.5, y) / uSrcSize).rgb;
    float ph = 6.2831853 * (px * uFsc + line * 0.25 + parity);
    float sub = yuv.y * sin(ph) + yuv.z * cos(ph); // modulated chroma on the luma wire
    float a = exp(-d * d * uSharp);
    float b = exp(-d * d * uChroma);
    accY += (yuv.x + uXtalk * sub) * a; wY += a;
    accC += yuv.yz * b; wC += b;
    edge += yuv.x * d * exp(-d * d * 1.2);         // luma detail at the subcarrier
  }
  float Y = accY / wY;
  vec2 C = accC / wC;
  // Cross-colour: fine luma detail decoded as false colour (rainbows).
  float ph0 = 6.2831853 * (fx * uFsc + line * 0.25 + parity);
  C += uXtalk * 0.9 * edge * vec2(sin(ph0), cos(ph0));
  if (uGhost > 0.0) {
    vec3 g = toYUV * texture2D(uSrc, vec2(floor(fx - uGhostOff) + 0.5, y) / uSrcSize).rgb;
    Y += uGhost * (g.x - 0.3);
    C += uGhost * 0.5 * g.yz;
  }
  float n1 = hash(vec3(floor(fx * 1.7), line, frame)) - 0.5;
  float n2 = hash(vec3(floor(fx * 0.6), line + 0.37, frame)) - 0.5;
  Y += uNoise * (n1 * 1.6 + n2 * 0.6);
  C += uNoise * 0.5 * vec2(n2, n1);
  Y *= 1.0 - uHum * (0.5 + 0.5 * sin(6.2831853 * (line / 312.0 - uTime * 0.35)));
  float ct = cos(uTint), st = sin(uTint);
  C = mat2(ct, st, -st, ct) * C * uSat;
  Y = Y * uGain + uBlack;
  C *= uGain;
  return clamp(toRGB * vec3(Y, C), 0.0, 1.0);
}

void main() {
  float m = texture2D(uMask, vUV).r;
  if (m < 0.002 || uBeam.z <= 0.0) { gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0); return; }

  vec2 c = (vUV * 2.0 - 1.0) * uOverscan;
  c *= 1.0 + uCurv * vec2(c.y * c.y, c.x * c.x);
  c /= vec2(max(uBeam.y, 1e-4), max(uBeam.x, 1e-4));
  vec2 r = c * 0.5 + 0.5;
  if (r.x < 0.0 || r.x > 1.0 || r.y < 0.0 || r.y > 1.0) { gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0); return; }

  float sx = uWin.x + r.x * uWin.z - uHPos;
  float sy = uWin.y + r.y * uWin.w + uRoll;

  // Scanlines need >= ~2 device pixels per line to look like scanlines
  // rather than moire; fade them out below that.
  float pxPerLine = uOut.y * uBeam.x / (uWin.w * uOverscan);
  float scan = uScan * smoothstep(1.25, 2.3, pxPerLine);

  float l0 = floor(sy - 0.5);
  float t = sy - 0.5 - l0;
  vec3 gauss = vec3(0.0);
  vec3 c0 = vec3(0.0), c1 = vec3(0.0);
  for (int k = -1; k <= 2; k++) {
    float L = l0 + float(k);
    vec3 lc = lineColor(L, sx);
    vec3 lin = lc * lc;                      // signal -> light (gamma ~2)
    float lum = dot(lin, vec3(0.30, 0.59, 0.11));
    float sig = mix(0.27, 0.46, sqrt(lum));  // bright lines bloom taller
    float d = sy - (L + 0.5);
    gauss += lin * exp(-0.5 * d * d / (sig * sig)) * (0.94 / (sig * 2.5066));
    if (k == 0) c0 = lin;
    if (k == 1) c1 = lin;
  }
  vec3 flatc = mix(c0, c1, t);
  vec3 col = mix(flatc, gauss, scan);

  // Shadow mask: RGB slot triads, staggered every other triad column.
  vec2 fc = gl_FragCoord.xy;
  float tx = mod(fc.x, 3.0);
  float col3 = floor(fc.x / 3.0);
  float slot = step(0.75, fract((fc.y + mod(col3, 2.0) * 2.0) / 4.0));
  vec3 mk = tx < 1.0 ? vec3(1.0, 0.2, 0.2) : (tx < 2.0 ? vec3(0.2, 1.0, 0.2) : vec3(0.2, 0.2, 1.0));
  mk *= 1.0 - 0.45 * slot;
  float maskAmt = uMaskAmt * smoothstep(0.8, 2.0, pxPerLine / 1.5);
  col *= mix(vec3(1.0), mk * 2.05, maskAmt);

  col *= uBright * uBeam.z;
  // Power-off dot: brighter as it shrinks (same energy, smaller area).
  col *= 1.0 / max(0.12, sqrt(uBeam.x * uBeam.y));
  gl_FragColor = vec4(sqrt(clamp(col, 0.0, 4.0) / 4.0), 1.0); // store headroom
}`;

// Render targets are written with vUV.y = 0 at the top of the viewport,
// which is texture row t = 1, so every pass that *reads* a render target
// samples at (x, 1 - y) to undo that flip.
const FS_BLUR = `
precision highp float;
varying vec2 vUV;
uniform sampler2D uTex;
uniform vec2 uDir; // texel step
void main() {
  vec3 acc = vec3(0.0);
  float wsum = 0.0;
  for (int i = -6; i <= 6; i++) {
    float w = exp(-float(i * i) / 18.0);
    vec3 s = texture2D(uTex, vec2(vUV.x, 1.0 - vUV.y) + uDir * float(i)).rgb;
    acc += s * s * w; wsum += w;           // blur light, not signal
  }
  gl_FragColor = vec4(sqrt(acc / wsum), 1.0);
}`;

const FS_PERSIST = `
precision highp float;
varying vec2 vUV;
uniform sampler2D uCrt;
uniform sampler2D uGlow;
uniform sampler2D uPrev;
uniform float uGlowAmt;
uniform float uPersist;
void main() {
  vec2 fuv = vec2(vUV.x, 1.0 - vUV.y); // render targets are stored bottom-up
  vec3 a = texture2D(uCrt, fuv).rgb; a = a * a * 4.0;
  vec3 g = texture2D(uGlow, fuv).rgb; g = g * g * 4.0;
  vec3 p = texture2D(uPrev, fuv).rgb; p = p * p * 4.0;
  vec3 col = a + g * uGlowAmt;
  col = max(col, p * uPersist);
  gl_FragColor = vec4(sqrt(clamp(col, 0.0, 4.0) / 4.0), 1.0);
}`;

const FS_PRESENT = `
precision highp float;
varying vec2 vUV;
uniform sampler2D uImg;
uniform sampler2D uMask;
uniform float uGlass;   // room light on the glass (0 = dark room)
uniform float uVig;     // edge falloff (CRT) -- 0 for LCD
uniform float uBlack;   // LCD backlight bleed (black level), 0 for CRT
void main() {
  float m = texture2D(uMask, vUV).r;
  vec3 col = texture2D(uImg, vec2(vUV.x, 1.0 - vUV.y)).rgb; col = col * col * 4.0;
  vec2 c = vUV * 2.0 - 1.0;
  // Edge falloff: the beam lands at a glancing angle near the rim.
  float vig = 1.0 - uVig * pow(dot(c * vec2(0.9, 1.0), c * vec2(0.9, 1.0)) * 0.5, 1.5);
  col *= vig;
  // Unlit phosphor is grey-green, not black; plus a soft window reflection
  // high on the left and a darker rim where the glass curves away.
  // (Kept small: in a lit room it lifts every dark channel, and a
  // little goes a long way once gamma-encoded.)
  vec3 glass = vec3(0.010, 0.012, 0.011) * (1.05 - 0.45 * dot(c, c) * 0.5);
  vec2 h = (vUV - vec2(0.27, 0.17)) / vec2(0.30, 0.16);
  float spec = exp(-dot(h, h)) * 0.018;
  col += (glass + spec) * uGlass;
  col = col * (1.0 - uBlack) + uBlack;
  // Soft knee above 0.8 instead of a hard clip, so bloom saturates gently.
  vec3 over = max(col - 0.8, 0.0);
  col = min(col, 0.8) + 0.2 * (1.0 - exp(-over / 0.2));
  gl_FragColor = vec4(sqrt(col) * m, m);   // same gamma 2 the beam pass decoded with; premultiplied
}`;

function compile(gl, type, src) {
  const s = gl.createShader(type);
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
    throw new Error('shader compile failed: ' + gl.getShaderInfoLog(s));
  }
  return s;
}

function program(gl, fs) {
  const p = gl.createProgram();
  gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, VS));
  gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, fs));
  gl.bindAttribLocation(p, 0, 'aPos');
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error('link failed: ' + gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) {
    const info = gl.getActiveUniform(p, i);
    u[info.name] = gl.getUniformLocation(p, info.name);
  }
  return { p, u };
}

function makeTex(gl, w, h, filter) {
  const t = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, t);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  if (w) gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  return t;
}

function makeTarget(gl, w, h) {
  const tex = makeTex(gl, w, h, gl.LINEAR);
  const fb = gl.createFramebuffer();
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  return { tex, fb, w, h };
}

// The picture tube's physical properties (CRT mode), adjustable in the
// settings.
export const TUBE_DEFAULTS = {
  curvature: 0.055,
  overscan: 0.965,
  scanlines: 0.85,
  mask: 0.22,
  glow: 0.16,
  persistence: 0.30,
  glass: 1.0,       // room light reflected by the glass
};

// What the monitor's front controls do to the signal, at their centre
// positions (see monitor-2002.js for the knobs).
export const NEUTRAL_PICTURE = { hpos: 0, roll: 0, saturation: 1, tint: 0, black: 0, gain: 1 };

// How each video connection degrades the signal. Luma/chroma filters are
// exp(-d^2 * k) per pixel: larger k = sharper. Values chosen by eye to
// resemble the respective connection on a 1702/2002-class monitor or TV.
export const CONNECTIONS = {
  svideo:    { sharpness: 4.0, chroma: 0.40, xtalk: 0.00, noise: 0.006, ghost: 0.00, ghostOff: 0, jitter: 0.00, hum: 0.000 },
  composite: { sharpness: 2.6, chroma: 0.30, xtalk: 0.09, noise: 0.014, ghost: 0.025, ghostOff: 2.5, jitter: 0.04, hum: 0.000 },
  rf:        { sharpness: 1.5, chroma: 0.18, xtalk: 0.16, noise: 0.050, ghost: 0.09, ghostOff: 4.0, jitter: 0.25, hum: 0.025 },
};

// A modern flat panel: no tube geometry, no scanlines, near-instant pixels.
const LCD_OVERRIDES = { curvature: 0, overscan: 1.0, scanlines: 0, mask: 0, glow: 0.02, persistence: 0.05, glass: 0 };

export class CrtRenderer {
  constructor(canvas, maskImage) {
    this.canvas = canvas;
    this.params = { ...TUBE_DEFAULTS };
    this.picture = { ...NEUTRAL_PICTURE };
    this.mode = 'crt';
    this.connection = CONNECTIONS.composite;
    this.fsc = 0.5625;
    this.win = [50, 7, 402, 287];
    this.srcW = 0; this.srcH = 0;
    this.lastAvg = [0, 0, 0];
    // power state machine: 'off' | 'warming' | 'on' | 'collapsing'
    this.power = 'off';
    this.powerT0 = 0;
    const gl = canvas.getContext('webgl', { alpha: true, premultipliedAlpha: true, antialias: false, preserveDrawingBuffer: false });
    this.gl = gl;
    if (!gl) {
      this.ctx2d = canvas.getContext('2d');
      this.maskImage = maskImage;
      return;
    }
    this.progCrt = program(gl, FS_CRT);
    this.progBlur = program(gl, FS_BLUR);
    this.progPersist = program(gl, FS_PERSIST);
    this.progPresent = program(gl, FS_PRESENT);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    this.srcTex = makeTex(gl, 0, 0, gl.NEAREST);
    this.maskTex = makeTex(gl, 0, 0, gl.LINEAR);
    gl.bindTexture(gl.TEXTURE_2D, this.maskTex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, maskImage);
    this.rectTex = makeTex(gl, 0, 0, gl.NEAREST);
    gl.bindTexture(gl.TEXTURE_2D, this.rectTex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([255, 255, 255, 255]));
    this.targets = null;
  }

  get isWebGL() { return !!this.gl; }

  /// Match the backing store to the on-screen size (call after layout
  /// changes). `cssW/cssH` default to the canvas's own client size.
  resize(cssW, cssH) {
    const dpr = Math.min(window.devicePixelRatio || 1, 2.5);
    const w = Math.max(16, Math.round((cssW ?? this.canvas.clientWidth) * dpr));
    const h = Math.max(16, Math.round((cssH ?? this.canvas.clientHeight) * dpr));
    if (w === this.canvas.width && h === this.canvas.height && this.targets) return;
    this.canvas.width = w; this.canvas.height = h;
    const gl = this.gl;
    if (!gl) return;
    const qw = Math.max(8, w >> 2), qh = Math.max(8, h >> 2);
    this.targets = {
      crt: makeTarget(gl, w, h),
      b1: makeTarget(gl, qw, qh),
      b2: makeTarget(gl, qw, qh),
      p: [makeTarget(gl, w, h), makeTarget(gl, w, h)],
      cur: 0,
    };
  }

  setWindow(x0, y0, w, h) { this.win = [x0, y0, w, h]; }
  /// Tube properties (TUBE_DEFAULTS keys).
  setParams(p) { Object.assign(this.params, p); }
  /// Front-panel adjustments (NEUTRAL_PICTURE keys); a flat panel ignores
  /// them.
  setPicture(p) { Object.assign(this.picture, p); }
  /// 'crt' (curved glass, beam, mask) or 'lcd' (flat panel).
  setMode(mode) { this.mode = mode; }
  /// 'svideo' | 'composite' | 'rf' -- see CONNECTIONS.
  setConnection(name) { this.connection = CONNECTIONS[name] || CONNECTIONS.composite; }
  /// Colour subcarrier cycles per C64 pixel: PAL 0.5625, NTSC 0.4375.
  setSubcarrier(cyclesPerPixel) { this.fsc = cyclesPerPixel; }
  effectiveParams() { return this.mode === 'lcd' ? { ...this.params, ...LCD_OVERRIDES } : this.params; }

  /// Upload a new frame (RGBA8, w*h*4 bytes).
  setSource(rgba, w, h) {
    this.srcW = w; this.srcH = h;
    this.lastSrc = rgba;
    // Average colour of the visible window, for light spill onto the room.
    let r = 0, g = 0, b = 0, n = 0;
    const [x0, y0, ww, hh] = this.win;
    for (let y = y0; y < y0 + hh; y += 9) {
      for (let x = x0; x < x0 + ww; x += 11) {
        const i = (y * w + x) * 4;
        r += rgba[i]; g += rgba[i + 1]; b += rgba[i + 2]; n++;
      }
    }
    this.lastAvg = [r / n / 255, g / n / 255, b / n / 255];
    const gl = this.gl;
    if (!gl) return;
    gl.bindTexture(gl.TEXTURE_2D, this.srcTex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, rgba);
  }

  setPower(on, now = performance.now()) {
    if (on && (this.power === 'off' || this.power === 'collapsing')) { this.power = 'warming'; this.powerT0 = now; }
    if (!on && (this.power === 'on' || this.power === 'warming')) { this.power = 'collapsing'; this.powerT0 = now; }
  }

  /// [vertical squash, horizontal squash, intensity] for the current
  /// moment of the power animation.
  beamState(now) {
    const t = (now - this.powerT0) / 1000;
    if (this.mode === 'lcd') {
      // No tube: the panel just shows the picture (after a short scaler
      // sync delay) and goes dark at once.
      if (this.power === 'warming') { if (t > 0.6) this.power = 'on'; return [1, 1, t > 0.35 ? 1 : 0]; }
      if (this.power === 'collapsing') { this.power = 'off'; return [1, 1, 0]; }
    }
    switch (this.power) {
      case 'off': return [1, 1, 0];
      case 'on': return [1, 1, 1];
      case 'warming': {
        // Cathode heats up: picture fades in over ~1.6 s, slightly
        // expanded at first (low EHT), settling to full size.
        if (t > 1.8) { this.power = 'on'; return [1, 1, 1]; }
        const k = Math.min(1, Math.max(0, (t - 0.25) / 1.4));
        const e = k * k * (3 - 2 * k);
        return [1.02 - 0.02 * e, 1.02 - 0.02 * e, e];
      }
      case 'collapsing': {
        // Deflection dies first: vertical collapses to a line, the line
        // shrinks to a dot, and the dot fades as the EHT bleeds away.
        if (t > 1.6) { this.power = 'off'; return [1, 1, 0]; }
        const v = Math.max(0.004, 1 - t / 0.08);
        const h = t < 0.08 ? 1 : Math.max(0.006, 1 - (t - 0.08) / 0.18);
        const i = t < 0.26 ? 1 : Math.max(0, 1 - (t - 0.26) / 1.3);
        return [v, h, i * i];
      }
    }
    return [1, 1, 1];
  }

  /// Average screen colour * beam intensity (for LED-style light spill).
  glowColor(now = performance.now()) {
    const z = this.beamState(now)[2];
    return this.lastAvg.map(c => c * z);
  }

  render(now = performance.now()) {
    const gl = this.gl;
    const P = this.effectiveParams();
    const S = this.connection;
    if (!gl) return this.render2d(now);
    if (!this.targets) this.resize();
    const T = this.targets;
    const beam = this.beamState(now);
    const W = this.canvas.width, H = this.canvas.height;

    // 1. CRT beam pass
    gl.bindFramebuffer(gl.FRAMEBUFFER, T.crt.fb);
    gl.viewport(0, 0, W, H);
    let pr = this.progCrt;
    gl.useProgram(pr.p);
    gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, this.srcTex);
    gl.uniform1i(pr.u.uSrc, 0);
    const mask = this.mode === 'lcd' ? this.rectTex : this.maskTex;
    gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, mask);
    gl.uniform1i(pr.u.uMask, 1);
    gl.uniform2f(pr.u.uSrcSize, this.srcW || 1, this.srcH || 1);
    gl.uniform4f(pr.u.uWin, ...this.win);
    gl.uniform2f(pr.u.uOut, W, H);
    gl.uniform1f(pr.u.uCurv, P.curvature);
    gl.uniform1f(pr.u.uOverscan, P.overscan);
    gl.uniform1f(pr.u.uScan, P.scanlines);
    gl.uniform1f(pr.u.uMaskAmt, P.mask);
    gl.uniform1f(pr.u.uSharp, this.mode === 'lcd' && S === CONNECTIONS.svideo ? 6.0 : S.sharpness);
    gl.uniform1f(pr.u.uChroma, S.chroma);
    gl.uniform1f(pr.u.uTime, now / 1000);
    gl.uniform1f(pr.u.uFsc, this.fsc);
    gl.uniform1f(pr.u.uXtalk, S.xtalk);
    gl.uniform1f(pr.u.uNoise, S.noise);
    gl.uniform1f(pr.u.uGhost, S.ghost);
    gl.uniform1f(pr.u.uGhostOff, S.ghostOff);
    gl.uniform1f(pr.u.uJitter, S.jitter);
    gl.uniform1f(pr.u.uHum, S.hum);
    const pic = this.mode === 'lcd' ? NEUTRAL_PICTURE : this.picture;
    const [, wy, , wh] = this.win;
    gl.uniform1f(pr.u.uBright, this.srcW ? 1.05 : 0);
    gl.uniform1f(pr.u.uHPos, pic.hpos);
    gl.uniform1f(pr.u.uRoll, pic.roll);
    gl.uniform1f(pr.u.uFrameH, this.srcH || 312);
    gl.uniform2f(pr.u.uBlank, pic.roll ? wy - 2 : -1e4, pic.roll ? wy + wh + 2 : 1e4);
    gl.uniform1f(pr.u.uSat, pic.saturation);
    gl.uniform1f(pr.u.uTint, pic.tint);
    gl.uniform1f(pr.u.uBlack, pic.black);
    gl.uniform1f(pr.u.uGain, pic.gain);
    gl.uniform3f(pr.u.uBeam, beam[0], beam[1], beam[2]);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);

    // 2. glow: separable blur at quarter resolution
    pr = this.progBlur;
    gl.useProgram(pr.p);
    gl.bindFramebuffer(gl.FRAMEBUFFER, T.b1.fb);
    gl.viewport(0, 0, T.b1.w, T.b1.h);
    gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, T.crt.tex);
    gl.uniform1i(pr.u.uTex, 0);
    gl.uniform2f(pr.u.uDir, 1.5 / T.b1.w, 0);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    gl.bindFramebuffer(gl.FRAMEBUFFER, T.b2.fb);
    gl.bindTexture(gl.TEXTURE_2D, T.b1.tex);
    gl.uniform2f(pr.u.uDir, 0, 1.5 / T.b2.h);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);

    // 3. persistence (ping-pong)
    const prev = T.p[T.cur], next = T.p[1 - T.cur];
    T.cur = 1 - T.cur;
    pr = this.progPersist;
    gl.useProgram(pr.p);
    gl.bindFramebuffer(gl.FRAMEBUFFER, next.fb);
    gl.viewport(0, 0, W, H);
    gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, T.crt.tex);
    gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, T.b2.tex);
    gl.activeTexture(gl.TEXTURE2); gl.bindTexture(gl.TEXTURE_2D, prev.tex);
    gl.uniform1i(pr.u.uCrt, 0); gl.uniform1i(pr.u.uGlow, 1); gl.uniform1i(pr.u.uPrev, 2);
    gl.uniform1f(pr.u.uGlowAmt, P.glow);
    gl.uniform1f(pr.u.uPersist, P.persistence);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);

    // 4. present onto the glass
    pr = this.progPresent;
    gl.useProgram(pr.p);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, W, H);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, next.tex);
    gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, mask);
    gl.uniform1i(pr.u.uImg, 0); gl.uniform1i(pr.u.uMask, 1);
    gl.uniform1f(pr.u.uGlass, P.glass);
    gl.uniform1f(pr.u.uVig, this.mode === 'lcd' ? 0 : 0.22);
    gl.uniform1f(pr.u.uBlack, this.mode === 'lcd' && this.power !== 'off' ? 0.012 : 0);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }

  render2d(now) {
    const ctx = this.ctx2d, W = this.canvas.width, H = this.canvas.height;
    ctx.globalCompositeOperation = 'source-over';
    ctx.clearRect(0, 0, W, H);
    ctx.fillStyle = '#101211';
    ctx.fillRect(0, 0, W, H);
    const z = this.beamState(now)[2];
    if (this.lastSrc && z > 0) {
      if (!this.tmp || this.tmp.width !== this.srcW) {
        this.tmp = document.createElement('canvas');
        this.tmp.width = this.srcW; this.tmp.height = this.srcH;
      }
      this.tmp.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(this.lastSrc), this.srcW, this.srcH), 0, 0);
      const [x0, y0, w, h] = this.win;
      ctx.imageSmoothingEnabled = true;
      ctx.globalAlpha = z;
      ctx.drawImage(this.tmp, x0, y0, w, h, 0, 0, W, H);
      ctx.globalAlpha = 1;
    }
    if (this.mode !== 'lcd') {
      ctx.globalCompositeOperation = 'destination-in';
      ctx.drawImage(this.maskImage, 0, 0, W, H);
      ctx.globalCompositeOperation = 'source-over';
    }
  }
}
