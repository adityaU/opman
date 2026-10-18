import type { PerspectiveCamera, Vector3 } from "three";

/** Place the camera so the whole box fits, looking at its centre from a pleasant angle. */
export function fitCamera(camera: PerspectiveCamera, center: Vector3, size: Vector3): number {
  const radius = Math.max(size.length() / 2, 1e-3);
  const fov = (camera.fov * Math.PI) / 180;
  const distance = (radius / Math.sin(fov / 2)) * 1.1;
  camera.position.set(center.x + distance * 0.6, center.y + distance * 0.5, center.z + distance * 0.62);
  camera.near = distance / 100;
  camera.far = distance * 100;
  camera.lookAt(center);
  camera.updateProjectionMatrix();
  return distance;
}
