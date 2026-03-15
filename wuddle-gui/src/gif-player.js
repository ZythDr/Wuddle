/**
 * GIF canvas player — decodes animated GIFs and plays them at the correct
 * frame rate on a <canvas>.  Fixes WebKitGTK's broken GIF timing.
 *
 * Usage:  import { replaceGifsWithCanvas } from "./gif-player.js";
 *         replaceGifsWithCanvas(containerEl);
 */
import { parseGIF, decompressFrames } from "./vendor/gifuct.bundle.js";

const MIN_DELAY = 20; // ms – frames below this threshold trigger the fix

/**
 * Scan `container` for <img> whose src ends in .gif (case-insensitive),
 * fetch + decode each one, and replace with a <canvas>-based player when
 * the GIF has at least one frame with a suspiciously low delay.
 */
export function replaceGifsWithCanvas(container) {
  const imgs = [...container.querySelectorAll("img[src]")].filter((img) => {
    const src = img.getAttribute("src") || "";
    return /\.gif(\?.*)?$/i.test(src);
  });
  for (const img of imgs) {
    processGifImg(img);
  }
}

async function processGifImg(img) {
  const src = img.getAttribute("src");
  if (!src) return;

  let buf;
  try {
    const resp = await fetch(src);
    if (!resp.ok) return; // leave the <img> as-is
    buf = await resp.arrayBuffer();
  } catch (_) {
    return; // network error — leave as-is
  }

  let frames;
  try {
    const gif = parseGIF(buf);
    frames = decompressFrames(gif, true);
  } catch (_) {
    return; // corrupt / unsupported GIF — leave as-is
  }

  if (!frames || frames.length < 2) return; // static GIF — nothing to fix

  // Check if any frame has a problematic delay
  const needsFix = frames.some((f) => f.delay < MIN_DELAY);
  if (!needsFix) return; // browser handles it fine

  const { width, height } = frames[0].dims;

  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  // Inherit the same sizing constraints as the <img>
  canvas.style.maxWidth = "100%";
  canvas.style.height = "auto";
  canvas.style.borderRadius = "4px";
  canvas.style.display = img.style.display || "";

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  // Full-frame composite buffer (handles dispose-to-background, etc.)
  const fullFrame = ctx.createImageData(width, height);

  // Replace the <img> in the DOM
  img.replaceWith(canvas);

  // Start animation loop
  animateFrames(canvas, ctx, frames, fullFrame);
}

function animateFrames(canvas, ctx, frames, fullFrame) {
  let idx = 0;
  let disposed = false;

  function renderFrame() {
    // If the canvas was removed from the DOM, stop the loop
    if (!canvas.isConnected) return;

    const frame = frames[idx];
    const { width: fw, height: fh, left, top } = frame.dims;
    const patchData = frame.patch;

    // Compose the frame patch onto the full-frame buffer
    for (let y = 0; y < fh; y++) {
      for (let x = 0; x < fw; x++) {
        const si = (y * fw + x) * 4;
        const di = ((top + y) * fullFrame.width + (left + x)) * 4;
        // Only draw non-transparent pixels (respect GIF transparency)
        if (patchData[si + 3] !== 0) {
          fullFrame.data[di] = patchData[si];
          fullFrame.data[di + 1] = patchData[si + 1];
          fullFrame.data[di + 2] = patchData[si + 2];
          fullFrame.data[di + 3] = patchData[si + 3];
        }
      }
    }

    ctx.putImageData(fullFrame, 0, 0);

    // Handle disposal method for next frame
    const disposalType = frame.disposalType;
    disposed = false;
    if (disposalType === 2) {
      // Restore to background — clear the frame region
      for (let y = 0; y < fh; y++) {
        for (let x = 0; x < fw; x++) {
          const di = ((top + y) * fullFrame.width + (left + x)) * 4;
          fullFrame.data[di] = 0;
          fullFrame.data[di + 1] = 0;
          fullFrame.data[di + 2] = 0;
          fullFrame.data[di + 3] = 0;
        }
      }
      disposed = true;
    }
    // disposalType 3 (restore to previous) is rare — treat as keep

    // Use the actual delay, with a sane minimum clamp
    const delay = Math.max(frame.delay || 100, 10);

    idx = (idx + 1) % frames.length;

    // If looping back to start, clear the composite buffer
    if (idx === 0) {
      const d = fullFrame.data;
      for (let i = 0; i < d.length; i++) d[i] = 0;
    }

    setTimeout(renderFrame, delay);
  }

  renderFrame();
}
