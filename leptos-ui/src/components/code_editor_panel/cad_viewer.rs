//! 3D model viewer using Three.js (loaded via CDN).
//! Supports STL, OBJ, GLTF/GLB, PLY, 3MF, FBX, DAE formats.
//! All interaction (zoom, pan, rotate) handled by Three.js OrbitControls.

use wasm_bindgen::prelude::*;

/// Initialize a Three.js scene in the given container element.
/// `url` is the raw file URL, `extension` is the file extension (e.g. "stl").
/// Returns a cleanup function handle (stored in JS, called on unmount).
pub fn init_3d_viewer(container_id: &str, url: &str, extension: &str) {
    let _ = init_viewer_js(
        JsValue::from_str(container_id),
        JsValue::from_str(url),
        JsValue::from_str(extension),
    );
}

/// Dispose the Three.js scene for the given container.
pub fn dispose_3d_viewer(container_id: &str) {
    dispose_viewer_js(JsValue::from_str(container_id));
}

#[wasm_bindgen(inline_js = r#"
const viewers = new Map();

export function init_viewer_js(containerId, url, ext) {
  const THREE = window.__THREE;
  const OrbitControls = window.__THREE_OrbitControls;
  const Loaders = window.__THREE_Loaders;
  if (!THREE || !OrbitControls || !Loaders) {
    console.error('[CAD] Three.js not loaded');
    return;
  }

  const container = document.getElementById(containerId);
  if (!container) return;

  // Clean up previous viewer if any
  if (viewers.has(containerId)) dispose_viewer_js(containerId);

  const w = container.clientWidth || 600;
  const h = container.clientHeight || 400;

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x1a1a2e);

  // Grid helper
  const grid = new THREE.GridHelper(20, 20, 0x444466, 0x333355);
  scene.add(grid);

  // Lights
  const ambient = new THREE.AmbientLight(0xffffff, 0.6);
  scene.add(ambient);
  const dirLight = new THREE.DirectionalLight(0xffffff, 0.8);
  dirLight.position.set(5, 10, 7);
  scene.add(dirLight);
  const dirLight2 = new THREE.DirectionalLight(0xffffff, 0.3);
  dirLight2.position.set(-5, -3, -5);
  scene.add(dirLight2);

  // Camera
  const camera = new THREE.PerspectiveCamera(50, w / h, 0.01, 1000);
  camera.position.set(3, 3, 3);

  // Renderer
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
  renderer.setSize(w, h);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  container.appendChild(renderer.domElement);

  // Controls
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.1;
  controls.enablePan = true;
  controls.enableZoom = true;

  // Resize observer
  let animId = 0;
  const resizeObs = new ResizeObserver(() => {
    const cw = container.clientWidth;
    const ch = container.clientHeight;
    if (cw > 0 && ch > 0) {
      camera.aspect = cw / ch;
      camera.updateProjectionMatrix();
      renderer.setSize(cw, ch);
    }
  });
  resizeObs.observe(container);

  // Animation loop
  function animate() {
    animId = requestAnimationFrame(animate);
    controls.update();
    renderer.render(scene, camera);
  }
  animate();

  // Store for cleanup
  viewers.set(containerId, { scene, camera, renderer, controls, resizeObs, animId });

  // Set loading state
  const loadingEl = container.querySelector('.cad-loading');

  // Choose loader by extension
  const e = ext.toLowerCase();
  let loader;
  if (e === 'stl') loader = new Loaders.STLLoader();
  else if (e === 'obj') loader = new Loaders.OBJLoader();
  else if (e === 'gltf' || e === 'glb') loader = new Loaders.GLTFLoader();
  else if (e === 'ply') loader = new Loaders.PLYLoader();
  else if (e === '3mf') loader = new Loaders.ThreeMFLoader();
  else if (e === 'fbx') loader = new Loaders.FBXLoader();
  else if (e === 'dae') loader = new Loaders.ColladaLoader();
  else {
    if (loadingEl) loadingEl.textContent = 'Unsupported format: ' + ext;
    return;
  }

  loader.load(url, (result) => {
    let object;
    if (e === 'stl' || e === 'ply') {
      // BufferGeometry returned
      const geom = result;
      geom.computeVertexNormals();
      const mat = new THREE.MeshStandardMaterial({
        color: 0x7799cc,
        metalness: 0.3,
        roughness: 0.6,
        flatShading: false,
      });
      object = new THREE.Mesh(geom, mat);
    } else if (e === 'gltf' || e === 'glb') {
      object = result.scene;
    } else if (e === 'dae') {
      object = result.scene;
    } else {
      object = result;
    }

    // Center and scale to fit
    const box = new THREE.Box3().setFromObject(object);
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z);
    if (maxDim > 0) {
      const scale = 4 / maxDim;
      object.scale.multiplyScalar(scale);
      box.setFromObject(object);
      box.getCenter(center);
    }
    object.position.sub(center);
    scene.add(object);

    // Fit camera
    const fitBox = new THREE.Box3().setFromObject(object);
    const fitSize = fitBox.getSize(new THREE.Vector3());
    const fitMax = Math.max(fitSize.x, fitSize.y, fitSize.z);
    const dist = fitMax / (2 * Math.tan((camera.fov * Math.PI) / 360));
    camera.position.set(dist * 0.8, dist * 0.6, dist * 0.8);
    controls.target.set(0, fitSize.y * 0.2, 0);
    controls.update();

    if (loadingEl) loadingEl.style.display = 'none';
  }, undefined, (err) => {
    console.error('[CAD] Load error:', err);
    if (loadingEl) loadingEl.textContent = 'Failed to load model';
  });
}

export function dispose_viewer_js(containerId) {
  const v = viewers.get(containerId);
  if (!v) return;
  cancelAnimationFrame(v.animId);
  v.resizeObs.disconnect();
  v.controls.dispose();
  v.renderer.dispose();
  if (v.renderer.domElement && v.renderer.domElement.parentNode) {
    v.renderer.domElement.parentNode.removeChild(v.renderer.domElement);
  }
  // Dispose scene objects
  v.scene.traverse((obj) => {
    if (obj.geometry) obj.geometry.dispose();
    if (obj.material) {
      if (Array.isArray(obj.material)) obj.material.forEach(m => m.dispose());
      else obj.material.dispose();
    }
  });
  viewers.delete(containerId);
}
"#)]
extern "C" {
    fn init_viewer_js(container_id: JsValue, url: JsValue, ext: JsValue);
    fn dispose_viewer_js(container_id: JsValue);
}
