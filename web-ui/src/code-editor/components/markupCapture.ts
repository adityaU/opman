/**
 * markupCapture — helpers for capturing preview content + annotations to clipboard.
 */

/** Serialize an SVG element and draw it onto a canvas context. */
export async function drawSvgToCanvas(
  svgEl: SVGSVGElement, ctx: CanvasRenderingContext2D, w: number, h: number,
): Promise<void> {
  try {
    const clone = svgEl.cloneNode(true) as SVGSVGElement;
    if (!clone.hasAttribute("width")) clone.setAttribute("width", String(w));
    if (!clone.hasAttribute("height")) clone.setAttribute("height", String(h));
    clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");

    const svgData = new XMLSerializer().serializeToString(clone);
    const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svgData)}`;
    const img = await loadImage(url);
    ctx.drawImage(img, 0, 0, w, h);
  } catch (e) {
    console.warn("[Markup] SVG capture failed:", e);
  }
}

/** Try to capture an iframe's visible content (same-origin only). */
export async function drawIframeToCanvas(
  iframe: HTMLIFrameElement, ctx: CanvasRenderingContext2D, parentRect: DOMRect,
): Promise<void> {
  try {
    const iDoc = iframe.contentDocument || iframe.contentWindow?.document;
    if (iDoc) {
      const svgInIframe = iDoc.querySelector("svg");
      if (svgInIframe) {
        const iRect = iframe.getBoundingClientRect();
        await drawSvgToCanvas(svgInIframe, ctx, iRect.width, iRect.height);
        return;
      }
    }
    // PDF or cross-origin — draw placeholder
    const iRect = iframe.getBoundingClientRect();
    const dx = iRect.left - parentRect.left;
    const dy = iRect.top - parentRect.top;
    ctx.fillStyle = "#2a2a3e";
    ctx.fillRect(dx, dy, iRect.width, iRect.height);
    ctx.fillStyle = "rgba(255,255,255,0.5)";
    ctx.font = "14px sans-serif";
    ctx.textAlign = "center";
    ctx.fillText("PDF Preview", dx + iRect.width / 2, dy + iRect.height / 2);
  } catch {
    console.warn("[Markup] iframe capture blocked (cross-origin)");
  }
}

/** Load an image from a URL as a promise. */
function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

/** Fit source dimensions into a box, returning draw coordinates. */
function fitToBox(srcW: number, srcH: number, boxW: number, boxH: number) {
  const srcAspect = srcW / srcH;
  const boxAspect = boxW / boxH;
  let dw: number, dh: number, dx: number, dy: number;
  if (srcAspect > boxAspect) {
    dw = boxW; dh = boxW / srcAspect;
    dx = 0; dy = (boxH - dh) / 2;
  } else {
    dh = boxH; dw = boxH * srcAspect;
    dx = (boxW - dw) / 2; dy = 0;
  }
  return { dx, dy, dw, dh };
}

/**
 * Capture the preview element + annotation canvas, returning a PNG blob.
 * Searches for img, video, WebGL canvas, SVG, or iframe inside the preview.
 */
export async function capturePreview(
  preview: HTMLElement,
  annotationCanvas: HTMLCanvasElement,
): Promise<Blob | null> {
  const rect = preview.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;

  const out = document.createElement("canvas");
  out.width = rect.width * dpr;
  out.height = rect.height * dpr;
  const ctx = out.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  // Background — computed style may be "transparent" or "rgba(0, 0, 0, 0)"
  const bg = getComputedStyle(preview).backgroundColor;
  const isTransparent = !bg || bg === "transparent" || bg === "rgba(0, 0, 0, 0)";
  ctx.fillStyle = isTransparent ? "#1a1a2e" : bg;
  ctx.fillRect(0, 0, rect.width, rect.height);

  // Find content elements
  const img = preview.querySelector("img") as HTMLImageElement | null;
  const video = preview.querySelector("video") as HTMLVideoElement | null;
  const glCanvas = preview.querySelector("canvas:not(.markup-canvas)") as HTMLCanvasElement | null;
  const svgEl = preview.querySelector("svg:not(button svg)") as SVGSVGElement | null;
  const iframe = preview.querySelector("iframe") as HTMLIFrameElement | null;

  if (img && img.complete && img.naturalWidth > 0) {
    const { dx, dy, dw, dh } = fitToBox(img.naturalWidth, img.naturalHeight, rect.width, rect.height);
    ctx.drawImage(img, dx, dy, dw, dh);
  } else if (video && video.videoWidth > 0) {
    const { dx, dy, dw, dh } = fitToBox(video.videoWidth, video.videoHeight, rect.width, rect.height);
    ctx.drawImage(video, dx, dy, dw, dh);
  } else if (glCanvas) {
    const cRect = glCanvas.getBoundingClientRect();
    ctx.drawImage(glCanvas, cRect.left - rect.left, cRect.top - rect.top, cRect.width, cRect.height);
  } else if (svgEl) {
    await drawSvgToCanvas(svgEl, ctx, rect.width, rect.height);
  } else if (iframe) {
    await drawIframeToCanvas(iframe, ctx, rect);
  }

  // Annotations on top
  ctx.drawImage(annotationCanvas, 0, 0, rect.width, rect.height);

  return new Promise((resolve) => out.toBlob(resolve, "image/png"));
}
