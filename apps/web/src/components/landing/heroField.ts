// A slow, dithered color field behind the hero. Same 4×4 Bayer dithering as
// the chef hat so the two read as one visual language. It follows the
// pointer gently, pauses offscreen, and renders a single still frame when
// reduced motion is requested.
import type * as THREEType from 'three';

const vertexShader = /* glsl */ `
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = vec4(position, 1.0);
  }
`;

const fragmentShader = /* glsl */ `
  precision highp float;
  uniform float uTime;
  uniform vec2 uResolution;
  uniform vec2 uPointer;
  uniform float uPixelRatio;
  uniform vec3 uColorA;
  uniform vec3 uColorB;
  uniform vec3 uColorC;
  uniform float uStrength;
  varying vec2 vUv;

  const mat4 THRESHOLDS = mat4(
    0.05882, 0.52941, 0.17647, 0.64706,
    0.76471, 0.29412, 0.88235, 0.41176,
    0.23529, 0.70588, 0.11765, 0.58824,
    0.94118, 0.47059, 0.82353, 0.35294
  );

  float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
  }

  float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(
      mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
      mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x),
      u.y
    );
  }

  float fbm(vec2 p) {
    float value = 0.0;
    float amplitude = 0.5;
    for (int octave = 0; octave < 5; octave++) {
      value += amplitude * noise(p);
      p = p * 2.03 + vec2(1.7, 9.2);
      amplitude *= 0.5;
    }
    return value;
  }

  void main() {
    vec2 aspect = vec2(uResolution.x / uResolution.y, 1.0);
    vec2 p = vUv * aspect * 2.2;
    float t = uTime * 0.045;

    // Domain-warped noise drifts slowly; the pointer bends it nearby.
    vec2 pointer = uPointer * aspect * 2.2;
    float pull = exp(-2.4 * length(p - pointer));
    vec2 warp = vec2(fbm(p + t), fbm(p - t + 4.3));
    float field = fbm(p + warp * 1.6 + pull * 0.6 + vec2(t * 2.0, -t));

    // Fade toward the edges and the bottom so content stays readable.
    float vignette = smoothstep(0.0, 0.55, vUv.y) * smoothstep(1.15, 0.35, length((vUv - vec2(0.5, 0.62)) * vec2(1.25, 1.6)));
    // Keep a calm, mostly clear space behind the headline and copy.
    float copyClearing = smoothstep(0.24, 0.7, length((vUv - vec2(0.5, 0.5)) * vec2(1.0, 2.1)));
    vignette *= mix(0.06, 1.0, copyClearing);
    float intensity = clamp((field - 0.42) * 1.7, 0.0, 1.0) * vignette * uStrength + pull * 0.12 * vignette;

    vec3 color = mix(uColorA, uColorB, smoothstep(0.25, 0.65, field));
    color = mix(color, uColorC, smoothstep(0.55, 0.9, field + pull * 0.3));

    float gridSize = max(uPixelRatio * 2.6, 1.0);
    vec2 cell = floor(gl_FragCoord.xy / gridSize);
    ivec2 index = ivec2(mod(cell, 4.0));
    float mark = step(THRESHOLDS[index.x][index.y], intensity);

    gl_FragColor = vec4(color, mark * 0.9);
  }
`;

function cssColor(THREE: typeof THREEType, name: string, fallback: string) {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return new THREE.Color(value || fallback);
}

export async function initializeHeroField(canvas: HTMLCanvasElement): Promise<() => void> {
  const THREE: typeof THREEType = await import('three');
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  let renderer: THREEType.WebGLRenderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: false, powerPreference: 'low-power' });
  } catch {
    canvas.remove();
    return () => {};
  }
  const pixelRatio = Math.min(window.devicePixelRatio, 1.5);
  renderer.setPixelRatio(pixelRatio);

  const uniforms = {
    uTime: { value: 0 },
    uResolution: { value: new THREE.Vector2(1, 1) },
    uPointer: { value: new THREE.Vector2(0.5, 0.65) },
    uPixelRatio: { value: pixelRatio },
    uColorA: { value: new THREE.Color() },
    uColorB: { value: new THREE.Color() },
    uColorC: { value: new THREE.Color() },
    uStrength: { value: 1 },
  };
  const applyTheme = () => {
    uniforms.uColorA.value = cssColor(THREE, '--pm-accent', '#ff896b');
    uniforms.uColorB.value = cssColor(THREE, '--pm-orange', '#ffb477');
    uniforms.uColorC.value = cssColor(THREE, '--pm-violet', '#b9a2ff');
    uniforms.uStrength.value = document.documentElement.dataset.theme === 'light' ? 0.42 : 0.8;
  };
  applyTheme();

  const scene = new THREE.Scene();
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  const material = new THREE.ShaderMaterial({ vertexShader, fragmentShader, uniforms, transparent: true });
  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), material);
  scene.add(mesh);

  const resize = () => {
    const { clientWidth, clientHeight } = canvas;
    renderer.setSize(clientWidth, clientHeight, false);
    uniforms.uResolution.value.set(clientWidth, clientHeight);
    if (reducedMotion) renderer.render(scene, camera);
  };
  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(canvas);
  resize();

  const target = new THREE.Vector2(0.5, 0.65);
  const onPointer = (event: PointerEvent) => {
    const bounds = canvas.getBoundingClientRect();
    target.set((event.clientX - bounds.left) / bounds.width, 1 - (event.clientY - bounds.top) / bounds.height);
  };
  window.addEventListener('pointermove', onPointer, { passive: true });

  const themeObserver = new MutationObserver(() => {
    applyTheme();
    if (reducedMotion) renderer.render(scene, camera);
  });
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });

  let frame = 0;
  let visible = true;
  const start = performance.now();
  const tick = () => {
    frame = requestAnimationFrame(tick);
    if (!visible) return;
    uniforms.uTime.value = (performance.now() - start) / 1000;
    uniforms.uPointer.value.lerp(target, 0.04);
    renderer.render(scene, camera);
  };

  const visibility = new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
  });
  visibility.observe(canvas);

  if (reducedMotion) {
    uniforms.uTime.value = 12;
    renderer.render(scene, camera);
  } else {
    tick();
  }
  canvas.dataset.ready = 'true';

  return () => {
    cancelAnimationFrame(frame);
    resizeObserver.disconnect();
    visibility.disconnect();
    themeObserver.disconnect();
    window.removeEventListener('pointermove', onPointer);
    material.dispose();
    mesh.geometry.dispose();
    renderer.dispose();
  };
}
