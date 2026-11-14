/**
 * The picture behind the stage. A CSS blur over the whole stage is repainted far too often in WebKitGTK, so the blur
 * is drawn once into a smaller copy of the picture instead. Where the canvas can't blur, the CSS blur is left to do it.
 */
export interface Background {
  url: string;
  /** Blur still to apply with CSS, in pixels. */
  cssBlur: number;
}

/** Wider than this, a blurred copy only costs memory. */
const MAX_WIDTH = 1600;

export async function makeBackground(blob: Blob, blur: number, stage: { width: number; height: number }): Promise<Background> {
  if (blur <= 0) return { url: URL.createObjectURL(blob), cssBlur: 0 };
  try {
    const img = await createImageBitmap(blob);
    const k = Math.min(1, MAX_WIDTH / img.width);
    const w = Math.round(img.width * k);
    const h = Math.round(img.height * k);
    const canvas = document.createElement('canvas');
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext('2d');
    if (!ctx || !('filter' in ctx)) throw new Error('no canvas filter');
    // The stage shows the picture scaled to cover it; blur by the same amount it would get on screen.
    const cover = Math.max(stage.width / w, stage.height / h) || 1;
    const px = blur / cover;
    ctx.filter = `blur(${px}px)`;
    // Drawn a little larger, so the edges blur into the picture rather than into transparency.
    const pad = px * 2;
    ctx.drawImage(img, -pad, -pad, w + pad * 2, h + pad * 2);
    img.close();
    const out = await new Promise<Blob | null>((res) => canvas.toBlob(res, 'image/jpeg', 0.92));
    if (!out) throw new Error('no blob');
    return { url: URL.createObjectURL(out), cssBlur: 0 };
  } catch {
    return { url: URL.createObjectURL(blob), cssBlur: blur };
  }
}
