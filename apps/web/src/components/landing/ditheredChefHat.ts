import type * as THREEType from 'three';
import chefHatUrl from '../../chefhat.png?url';

const MAX_PIXEL_RATIO = 2;
const CAMERA_DISTANCE = 4.4;
const CAMERA_FOV = 48;
const POINTER_DAMPING = 0.055;

type Bounds = {
  left: number;
  top: number;
  width: number;
  height: number;
};

const ditherVertexShader = /* glsl */ `
  varying vec2 vUv;

  void main() {
    vUv = uv;
    gl_Position = vec4(position, 1.0);
  }
`;

const ditherFragmentShader = /* glsl */ `
  uniform sampler2D tDiffuse;
  uniform float uPixelRatio;
  varying vec2 vUv;

  const mat4 THRESHOLDS = mat4(
    0.05882, 0.52941, 0.17647, 0.64706,
    0.76471, 0.29412, 0.88235, 0.41176,
    0.23529, 0.70588, 0.11765, 0.58824,
    0.94118, 0.47059, 0.82353, 0.35294
  );

  void main() {
    vec4 source = texture2D(tDiffuse, vUv);
    float luma = dot(source.rgb, vec3(0.2126, 0.7152, 0.0722));
    float gridSize = max(uPixelRatio * 2.6, 1.0);
    vec2 cell = floor(gl_FragCoord.xy / gridSize);
    ivec2 matrixPosition = ivec2(mod(cell, 4.0));
    float threshold = THRESHOLDS[matrixPosition.x][matrixPosition.y];
    float mark = step(threshold, luma);

    gl_FragColor = vec4(vec3(1.0), source.a * mark);
  }
`;

export type DitheredChefHatCleanup = () => void;

export const initializeDitheredChefHat = async (
  container: HTMLElement,
  canvas: HTMLCanvasElement,
): Promise<DitheredChefHatCleanup | undefined> => {
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return undefined;
  if (window.matchMedia('(pointer: coarse)').matches) return undefined;
  if (!('IntersectionObserver' in window) || !('ResizeObserver' in window)) return undefined;

  const THREE: typeof THREEType = await import('three');

  let renderer: THREEType.WebGLRenderer;

  try {
    renderer = new THREE.WebGLRenderer({
      alpha: true,
      antialias: false,
      canvas,
      powerPreference: 'low-power',
    });
  } catch {
    return undefined;
  }

  let texture: THREEType.Texture;

  try {
    texture = await new THREE.TextureLoader().loadAsync(chefHatUrl);
  } catch {
    renderer.dispose();
    return undefined;
  }

  let disposed = false;
  let visible = false;
  let frameId: number | undefined;
  let renderTarget: THREEType.WebGLRenderTarget | undefined;
  let lastFrameTime = performance.now();
  let elapsed = 0;
  let bounds: Bounds = { left: 0, top: 0, width: 0, height: 0 };
  const pointer = { x: 0, y: 0 };
  const smooth = { x: 0, y: 0 };

  texture.colorSpace = THREE.SRGBColorSpace;
  texture.minFilter = THREE.LinearFilter;
  texture.magFilter = THREE.LinearFilter;

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(CAMERA_FOV, 1, 0.1, 100);
  camera.position.z = CAMERA_DISTANCE;

  const floatingHat = new THREE.Group();
  floatingHat.rotation.set(-0.04, -0.12, 0.03);
  scene.add(floatingHat);
  const hatMaterial = new THREE.MeshBasicMaterial({
    alphaTest: 0.01,
    map: texture,
    side: THREE.DoubleSide,
    transparent: true,
  });
  const hat = new THREE.Mesh(new THREE.PlaneGeometry(3.4, 3.4), hatMaterial);
  floatingHat.add(hat);

  const outputScene = new THREE.Scene();
  const outputCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  const outputMaterial = new THREE.ShaderMaterial({
    depthTest: false,
    depthWrite: false,
    fragmentShader: ditherFragmentShader,
    transparent: true,
    uniforms: {
      tDiffuse: { value: null },
      uPixelRatio: { value: 1 },
    },
    vertexShader: ditherVertexShader,
  });
  const outputQuad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), outputMaterial);
  outputScene.add(outputQuad);

  renderer.setClearColor(0x000000, 0);
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const recordPointer = (event: PointerEvent) => {
    if (event.pointerType === 'touch' || bounds.width <= 0 || bounds.height <= 0) return;
    const inside =
      event.clientX >= bounds.left &&
      event.clientX <= bounds.left + bounds.width &&
      event.clientY >= bounds.top &&
      event.clientY <= bounds.top + bounds.height;
    if (!inside) {
      pointer.x = 0;
      pointer.y = 0;
      return;
    }
    pointer.x = ((event.clientX - bounds.left) / bounds.width) * 2 - 1;
    pointer.y = ((event.clientY - bounds.top) / bounds.height) * 2 - 1;
  };

  const resetPointer = () => {
    pointer.x = 0;
    pointer.y = 0;
  };

  const resize = () => {
    if (disposed) return;
    const rect = container.getBoundingClientRect();
    bounds = {
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
    };
    if (rect.width <= 0 || rect.height <= 0) return;

    const pixelRatio = Math.min(window.devicePixelRatio || 1, MAX_PIXEL_RATIO);
    camera.aspect = rect.width / rect.height;
    camera.position.z = CAMERA_DISTANCE;
    camera.updateProjectionMatrix();
    renderer.setPixelRatio(pixelRatio);
    renderer.setSize(rect.width, rect.height, false);

    renderTarget?.dispose();
    renderTarget = new THREE.WebGLRenderTarget(
      Math.max(1, Math.round(rect.width * pixelRatio)),
      Math.max(1, Math.round(rect.height * pixelRatio)),
      { depthBuffer: true },
    );
    outputMaterial.uniforms.tDiffuse.value = renderTarget.texture;
    outputMaterial.uniforms.uPixelRatio.value = pixelRatio;
  };

  const render = (elapsedSeconds: number, deltaSeconds: number) => {
    if (!renderTarget || disposed) return;

    if (deltaSeconds > 0) {
      const alpha = 1 - Math.pow(1 - POINTER_DAMPING, deltaSeconds * 60);
      smooth.x += (pointer.x - smooth.x) * alpha;
      smooth.y += (pointer.y - smooth.y) * alpha;
    }

    floatingHat.position.x = smooth.x * 0.16;
    floatingHat.position.y = Math.sin(elapsedSeconds * 0.72) * 0.07 + smooth.y * 0.08;
    floatingHat.rotation.x = -0.04 + Math.sin(elapsedSeconds * 0.38) * 0.025 + smooth.y * 0.42;
    floatingHat.rotation.y = -0.12 + Math.sin(elapsedSeconds * 0.46) * 0.08 + smooth.x * 0.62;
    floatingHat.rotation.z = 0.03 + Math.sin(elapsedSeconds * 0.38) * 0.025 + smooth.x * 0.035;

    camera.position.x = smooth.x * 0.34;
    camera.position.y = smooth.y * 0.22;
    camera.position.z = CAMERA_DISTANCE;
    camera.lookAt(camera.position.x * 0.45, camera.position.y * 0.45, 0);

    renderer.setRenderTarget(renderTarget);
    renderer.clear();
    renderer.render(scene, camera);
    renderer.setRenderTarget(null);
    renderer.clear();
    renderer.render(outputScene, outputCamera);
  };

  const renderFrame = (now: number) => {
    if (disposed) return;
    frameId = window.requestAnimationFrame(renderFrame);
    const delta = Math.min((now - lastFrameTime) / 1000, 1 / 30);
    lastFrameTime = now;
    if (!visible || document.visibilityState !== 'visible') return;

    elapsed += delta;
    render(elapsed, delta);
  };

  const startRendering = () => {
    if (frameId !== undefined || disposed) return;
    lastFrameTime = performance.now();
    frameId = window.requestAnimationFrame(renderFrame);
  };

  const stopRendering = () => {
    if (frameId === undefined) return;
    window.cancelAnimationFrame(frameId);
    frameId = undefined;
  };

  const resizeObserver = new ResizeObserver(resize);
  const intersectionObserver = new IntersectionObserver(
    ([entry]) => {
      visible = entry?.isIntersecting ?? false;
      if (visible) {
        startRendering();
        render(elapsed, 0);
      } else {
        stopRendering();
      }
    },
    { threshold: 0.01 },
  );
  const handleVisibilityChange = () => {
    if (document.visibilityState === 'visible') startRendering();
    else stopRendering();
  };

  resizeObserver.observe(container);
  intersectionObserver.observe(container);
  window.addEventListener('pointermove', recordPointer, { passive: true });
  window.addEventListener('blur', resetPointer);
  document.addEventListener('visibilitychange', handleVisibilityChange);

  resize();
  render(0, 0);
  container.classList.add('is-enhanced');
  startRendering();

  return () => {
    if (disposed) return;
    disposed = true;
    stopRendering();
    resizeObserver.disconnect();
    intersectionObserver.disconnect();
    window.removeEventListener('pointermove', recordPointer);
    window.removeEventListener('blur', resetPointer);
    document.removeEventListener('visibilitychange', handleVisibilityChange);
    container.classList.remove('is-enhanced');

    hat.geometry.dispose();
    hatMaterial.dispose();
    texture.dispose();
    outputQuad.geometry.dispose();
    outputMaterial.dispose();
    renderTarget?.dispose();
    renderer.renderLists.dispose();
    renderer.dispose();
    renderer.forceContextLoss();
  };
};
