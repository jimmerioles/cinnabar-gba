// Frame presentation for the browser arcade.
//
// Both renderers consume the RGBA8 buffer from the WASM module. WebGL2 is
// preferred because it can apply the LCD/CRT post-processing at the display
// resolution; a plain 2D canvas is the fallback, with the same presets applied
// as a CSS overlay by the page.

const SOURCE_WIDTH = 240;
const SOURCE_HEIGHT = 160;

export const PRESETS = {
  off: { scanline: 0.0, mask: 0.0, vignette: 0.05, curvature: 0.0 },
  lcd: { scanline: 0.16, mask: 0.025, vignette: 0.16, curvature: 0.0 },
  crt: { scanline: 0.3, mask: 0.09, vignette: 0.34, curvature: 0.35 },
};

const VERTEX_SHADER = `#version 300 es
in vec2 aPosition;
out vec2 vUv;

void main() {
  vUv = vec2(aPosition.x * 0.5 + 0.5, 0.5 - aPosition.y * 0.5);
  gl_Position = vec4(aPosition, 0.0, 1.0);
}
`;

const FRAGMENT_SHADER = `#version 300 es
precision highp float;

in vec2 vUv;
out vec4 outColor;

uniform sampler2D uTexture;
uniform vec2 uSourceSize;
uniform float uScanline;
uniform float uMask;
uniform float uVignette;
uniform float uCurvature;

vec2 curveUv(vec2 uv) {
  vec2 centered = uv * 2.0 - 1.0;
  vec2 offset = abs(centered.yx) * 0.12;
  centered += centered * offset * offset * uCurvature * 8.0;
  return centered * 0.5 + 0.5;
}

void main() {
  vec2 uv = vUv;
  if (uCurvature > 0.001) {
    uv = curveUv(uv);
  }
  if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
    outColor = vec4(0.0, 0.0, 0.0, 1.0);
    return;
  }

  vec3 color = texture(uTexture, uv).rgb;

  float row = floor(uv.y * uSourceSize.y);
  float scan = mix(1.0 - uScanline, 1.0, mod(row, 2.0));
  color *= scan;

  float channel = mod(floor(gl_FragCoord.x), 3.0);
  vec3 aperture = vec3(1.0);
  if (channel < 1.0) {
    aperture = vec3(1.0, 1.0 - uMask, 1.0 - uMask);
  } else if (channel < 2.0) {
    aperture = vec3(1.0 - uMask, 1.0, 1.0 - uMask);
  } else {
    aperture = vec3(1.0 - uMask, 1.0 - uMask, 1.0);
  }
  color *= aperture;

  vec2 centered = uv - 0.5;
  color *= 1.0 - uVignette * dot(centered, centered) * 1.6;

  outColor = vec4(color, 1.0);
}
`;

export function createRenderer(canvas, preset = "off") {
  let gl = null;
  try {
    gl = canvas.getContext("webgl2", {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      premultipliedAlpha: false,
      preserveDrawingBuffer: false,
    });
  } catch (error) {
    console.warn("cinnabar: WebGL2 unavailable", error);
  }

  if (gl) {
    try {
      return new WebGLRenderer(canvas, gl, preset);
    } catch (error) {
      console.warn("cinnabar: WebGL renderer failed, using the 2D fallback", error);
      const replacement = canvas.cloneNode(false);
      canvas.replaceWith(replacement);
      canvas = replacement;
    }
  }

  return new CanvasRenderer(canvas, preset);
}

class CanvasRenderer {
  constructor(canvas, preset) {
    this.kind = "2d";
    this.canvas = canvas;
    this.preset = normalizePreset(preset);
    this.context = canvas.getContext("2d", { alpha: false });
    if (!this.context) {
      throw new Error("neither WebGL2 nor 2D canvas is available");
    }
    canvas.width = SOURCE_WIDTH;
    canvas.height = SOURCE_HEIGHT;
    this.image = this.context.createImageData(SOURCE_WIDTH, SOURCE_HEIGHT);
  }

  resize(scale) {
    this.scale = scale;
    this.canvas.style.width = `${SOURCE_WIDTH * scale}px`;
    this.canvas.style.height = `${SOURCE_HEIGHT * scale}px`;
  }

  setPreset(preset) {
    this.preset = normalizePreset(preset);
  }

  draw(rgba) {
    this.image.data.set(rgba);
    this.context.putImageData(this.image, 0, 0);
  }
}

class WebGLRenderer {
  constructor(canvas, gl, preset) {
    this.kind = "webgl2";
    this.canvas = canvas;
    this.gl = gl;
    this.preset = normalizePreset(preset);
    this.scale = 1;

    this.program = createProgram(gl, VERTEX_SHADER, FRAGMENT_SHADER);
    this.locations = {
      position: gl.getAttribLocation(this.program, "aPosition"),
      sourceSize: gl.getUniformLocation(this.program, "uSourceSize"),
      scanline: gl.getUniformLocation(this.program, "uScanline"),
      mask: gl.getUniformLocation(this.program, "uMask"),
      vignette: gl.getUniformLocation(this.program, "uVignette"),
      curvature: gl.getUniformLocation(this.program, "uCurvature"),
      texture: gl.getUniformLocation(this.program, "uTexture"),
    };

    this.quad = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.quad);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
      gl.STATIC_DRAW,
    );

    this.texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      SOURCE_WIDTH,
      SOURCE_HEIGHT,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      null,
    );

    gl.useProgram(this.program);
    gl.uniform1i(this.locations.texture, 0);
    gl.uniform2f(this.locations.sourceSize, SOURCE_WIDTH, SOURCE_HEIGHT);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.quad);
    gl.enableVertexAttribArray(this.locations.position);
    gl.vertexAttribPointer(this.locations.position, 2, gl.FLOAT, false, 0, 0);

    this.resize(1);
    this.applyPreset();
  }

  resize(scale) {
    const width = Math.max(1, Math.round(SOURCE_WIDTH * scale));
    const height = Math.max(1, Math.round(SOURCE_HEIGHT * scale));
    this.scale = scale;
    if (this.canvas.width !== width || this.canvas.height !== height) {
      this.canvas.width = width;
      this.canvas.height = height;
    }
    this.canvas.style.width = `${width}px`;
    this.canvas.style.height = `${height}px`;
    this.gl.viewport(0, 0, width, height);
  }

  setPreset(preset) {
    this.preset = normalizePreset(preset);
    this.applyPreset();
  }

  applyPreset() {
    const gl = this.gl;
    const settings = PRESETS[this.preset] ?? PRESETS.off;
    gl.useProgram(this.program);
    gl.uniform1f(this.locations.scanline, settings.scanline);
    gl.uniform1f(this.locations.mask, settings.mask);
    gl.uniform1f(this.locations.vignette, settings.vignette);
    gl.uniform1f(this.locations.curvature, settings.curvature);
  }

  draw(rgba) {
    const gl = this.gl;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texSubImage2D(
      gl.TEXTURE_2D,
      0,
      0,
      0,
      SOURCE_WIDTH,
      SOURCE_HEIGHT,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      rgba,
    );
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }
}

function normalizePreset(preset) {
  return Object.prototype.hasOwnProperty.call(PRESETS, preset) ? preset : "off";
}

function createProgram(gl, vertexSource, fragmentSource) {
  const vertex = compileShader(gl, gl.VERTEX_SHADER, vertexSource);
  const fragment = compileShader(gl, gl.FRAGMENT_SHADER, fragmentSource);
  const program = gl.createProgram();
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program);
    gl.deleteProgram(program);
    throw new Error(`shader link failed: ${log}`);
  }
  return program;
}

function compileShader(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`shader compile failed: ${log}`);
  }
  return shader;
}
