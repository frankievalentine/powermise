import * as THREE from 'three';
import chefHatUrl from '../../chefhat.png?url';

const MAX_PIXEL_RATIO = 2;
const CAMERA_DISTANCE = 4.4;
const CAMERA_FOV = 48;

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
  if (!('IntersectionObserver' in window) || !('ResizeObserver' in window)) return undefined;

  let renderer: THREE.WebGLRenderer;

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

  let texture: THREE.Texture;

  try {
    texture = await new THREE.TextureLoader().loadAsync(chefHatUrl);
  } catch {
    renderer.dispose();
    return undefined;
  }

  let disposed = false;
  let visible = false;
  let frameId: number | undefined;
  let renderTarget: THREE.WebGLRenderTarget | undefined;

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

  const render = (elapsedSeconds: number) => {
    if (!renderTarget || disposed) return;

    floatingHat.position.y = Math.sin(elapsedSeconds * 0.72) * 0.07;
    floatingHat.rotation.y = -0.12 + Math.sin(elapsedSeconds * 0.46) * 0.08;
    floatingHat.rotation.z = 0.03 + Math.sin(elapsedSeconds * 0.38) * 0.025;

    renderer.setRenderTarget(renderTarget);
    renderer.clear();
    renderer.render(scene, camera);
    renderer.setRenderTarget(null);
    renderer.clear();
    renderer.render(outputScene, outputCamera);
  };

  const resize = () => {
    const { height, width } = container.getBoundingClientRect();
    if (width <= 0 || height <= 0 || disposed) return;

    const pixelRatio = Math.min(window.devicePixelRatio || 1, MAX_PIXEL_RATIO);
    camera.aspect = width / height;
    camera.position.z = CAMERA_DISTANCE;
    camera.updateProjectionMatrix();
    renderer.setPixelRatio(pixelRatio);
    renderer.setSize(width, height, false);

    renderTarget?.dispose();
    renderTarget = new THREE.WebGLRenderTarget(
      Math.max(1, Math.round(width * pixelRatio)),
      Math.max(1, Math.round(height * pixelRatio)),
      { depthBuffer: true },
    );
    outputMaterial.uniforms.tDiffuse.value = renderTarget.texture;
    outputMaterial.uniforms.uPixelRatio.value = pixelRatio;
  };

  const startedAt = performance.now();
  const renderFrame = () => {
    frameId = window.requestAnimationFrame(renderFrame);
    if (!visible || document.visibilityState !== 'visible') return;
    render((performance.now() - startedAt) / 1000);
  };

  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(container);
  const intersectionObserver = new IntersectionObserver(
    ([entry]) => {
      visible = entry?.isIntersecting ?? false;
      if (visible) render((performance.now() - startedAt) / 1000);
    },
    { threshold: 0.01 },
  );
  intersectionObserver.observe(container);

  resize();
  render(0);
  container.classList.add('is-enhanced');
  frameId = window.requestAnimationFrame(renderFrame);

  return () => {
    if (disposed) return;
    disposed = true;

    if (frameId !== undefined) window.cancelAnimationFrame(frameId);
    resizeObserver.disconnect();
    intersectionObserver.disconnect();
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
