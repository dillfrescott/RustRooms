// Browser-side avatar resizing.
// - Oversized GIFs are captured frame-by-frame (canvas + rAF), downscaled and
//   re-encoded with the vendored gifenc encoder to fit the server's 2 MiB
//   avatar cap while keeping the animation.
// - Oversized still images are re-encoded to a smaller JPEG.
// All of this runs client-side and ships inside the server binary (include_str!).
(function () {
    'use strict';

    // Server cap (MAX_AVATAR_DATA_LEN) applies to the data URL string itself;
    // keep a margin for the data:image/gif;base64, prefix and JSON envelope.
    const MAX_AVATAR_DATA_URL_LEN = 2 * 1024 * 1024 - 100 * 1024;
    const GIF_MAX_CAPTURE_DIM = 400;
    const GIF_MAX_FRAMES = 150;
    const GIF_QUIET_MS = 2000;
    const MIN_FRAME_DELAY_MS = 20;
    const MAX_FRAME_DELAY_MS = 5000;

    function readFileAsDataUrl(file) {
        return new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result);
            reader.onerror = () => reject(reader.error || new Error('Failed to read file'));
            reader.readAsDataURL(file);
        });
    }

    // FNV-1a 32-bit hash over RGBA pixels; also flags any translucent pixel.
    function hashAndCheckAlpha(data) {
        let hash = 0x811c9dc5;
        let hasAlpha = false;
        const n = data.length;
        for (let i = 0; i < n; i++) {
            hash ^= data[i];
            hash = Math.imul(hash, 16777619) >>> 0;
            if ((i & 3) === 3 && data[i] < 255) hasAlpha = true;
        }
        return { hash, hasAlpha };
    }

    // Captures an animated GIF frame-by-frame by rendering it to a canvas on
    // each animation frame and recording distinct frames plus how long each
    // was displayed. Loop duplicates are merged back into their original
    // frame. Stops after one full loop, after GIF_QUIET_MS without a change
    // (static tail / long hold), or at the frame cap.
    function captureGifFrames(dataUrl, maxDim) {
        return new Promise((resolve, reject) => {
            const img = new Image();
            img.onerror = () => reject(new Error('Failed to decode GIF'));
            img.onload = () => {
                const scale = Math.min(maxDim / img.naturalWidth, maxDim / img.naturalHeight, 1);
                const width = Math.max(1, Math.round(img.naturalWidth * scale));
                const height = Math.max(1, Math.round(img.naturalHeight * scale));
                const canvas = document.createElement('canvas');
                canvas.width = width;
                canvas.height = height;
                const ctx = canvas.getContext('2d', { willReadFrequently: true });

                const frames = [];
                const hashes = [];
                let hasAlpha = false;
                let currentIndex = -1;
                let lastChange = 0;

                const finish = (now) => {
                    if (currentIndex >= 0) {
                        frames[currentIndex].delay += Math.max(0, now - lastChange);
                    }
                    if (frames.length === 0) {
                        reject(new Error('No frames captured'));
                    } else {
                        resolve({ frames, width, height, hasAlpha });
                    }
                };

                const tick = () => {
                    const now = performance.now();
                    ctx.drawImage(img, 0, 0, width, height);
                    const data = ctx.getImageData(0, 0, width, height).data;
                    const result = hashAndCheckAlpha(data);
                    hasAlpha = hasAlpha || result.hasAlpha;
                    if (result.hash === hashes[currentIndex]) {
                        if (now - lastChange >= GIF_QUIET_MS || frames.length >= GIF_MAX_FRAMES) {
                            finish(now);
                            return;
                        }
                    } else {
                        if (currentIndex >= 0) {
                            frames[currentIndex].delay += Math.max(0, now - lastChange);
                        }
                        lastChange = now;
                        const dup = hashes.indexOf(result.hash);
                        if (dup === 0) {
                            // Completed one full loop of the animation.
                            currentIndex = dup;
                            finish(now);
                            return;
                        }
                        if (dup >= 0) {
                            currentIndex = dup;
                        } else {
                            currentIndex = frames.length;
                            frames.push({ data: new Uint8ClampedArray(data), delay: 0 });
                            hashes.push(result.hash);
                        }
                    }
                    if (frames.length >= GIF_MAX_FRAMES) {
                        finish(now);
                        return;
                    }
                    requestAnimationFrame(tick);
                };
                requestAnimationFrame(tick);
            };
            img.src = dataUrl;
        });
    }

    function clampDelay(ms) {
        return Math.max(MIN_FRAME_DELAY_MS, Math.min(MAX_FRAME_DELAY_MS, ms));
    }

    function bytesToDataUrl(bytes, mime) {
        let binary = '';
        const chunk = 0x8000;
        for (let i = 0; i < bytes.length; i += chunk) {
            binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
        }
        return 'data:' + mime + ';base64,' + btoa(binary);
    }

    function encodeGifFrames(frames, width, height, hasAlpha) {
        const enc = globalThis.__gifenc;
        const gif = new enc.GIFEncoder();
        frames.forEach((frame) => {
            const rgba = new Uint8ClampedArray(frame.data);
            const format = hasAlpha ? 'rgba4444' : 'rgb565';
            const palette = enc.quantize(rgba, 256, { format });
            const index = enc.applyPalette(rgba, palette, format);
            const opts = { palette, delay: clampDelay(frame.delay), repeat: 0 };
            if (hasAlpha) {
                const transparentIndex = palette.findIndex((c) => c.length >= 4 && c[3] === 0);
                if (transparentIndex >= 0) {
                    opts.transparent = true;
                    opts.transparentIndex = transparentIndex;
                    // applyPalette balances alpha vs RGB distance, so fully
                    // transparent pixels could map to an opaque entry; force
                    // them to the transparent index (GIF transparency is
                    // binary, so this is also the exact desired output).
                    for (let i = 0; i < frame.data.length; i += 4) {
                        if (frame.data[i + 3] === 0) {
                            index[i >> 2] = transparentIndex;
                        }
                    }
                }
            }
            gif.writeFrame(index, width, height, opts);
        });
        gif.finish();
        return bytesToDataUrl(gif.bytes(), 'image/gif');
    }

    function downscaleGifFrames(frames, srcW, srcH, dstW, dstH) {
        const src = document.createElement('canvas');
        src.width = srcW;
        src.height = srcH;
        const sctx = src.getContext('2d');
        const dst = document.createElement('canvas');
        dst.width = dstW;
        dst.height = dstH;
        const dctx = dst.getContext('2d');
        return frames.map((frame) => {
            sctx.putImageData(new ImageData(frame.data, srcW, srcH), 0, 0);
            dctx.clearRect(0, 0, dstW, dstH);
            dctx.drawImage(src, 0, 0, dstW, dstH);
            return {
                data: new Uint8ClampedArray(dctx.getImageData(0, 0, dstW, dstH).data),
                delay: frame.delay,
            };
        });
    }

    async function resizeGifDataUrlToFit(dataUrl) {
        const captured = await captureGifFrames(dataUrl, GIF_MAX_CAPTURE_DIM);
        const dims = [400, 320, 256, 200, 160, 128];
        for (const dim of dims) {
            if (dim > captured.width && dim > captured.height) continue;
            const scale = Math.min(dim / captured.width, dim / captured.height, 1);
            const width = Math.max(1, Math.round(captured.width * scale));
            const height = Math.max(1, Math.round(captured.height * scale));
            const frames = (width === captured.width && height === captured.height)
                ? captured.frames
                : downscaleGifFrames(captured.frames, captured.width, captured.height, width, height);
            const out = encodeGifFrames(frames, width, height, captured.hasAlpha);
            if (out.length <= MAX_AVATAR_DATA_URL_LEN) {
                return { avatar: out, isGif: true };
            }
        }
        // Practically unreachable fallback: first frame as a small JPEG.
        const canvas = document.createElement('canvas');
        canvas.width = captured.width;
        canvas.height = captured.height;
        const ctx = canvas.getContext('2d');
        ctx.putImageData(new ImageData(captured.frames[0].data, captured.width, captured.height), 0, 0);
        let jpeg = canvas.toDataURL('image/jpeg', 0.7);
        if (jpeg.length > MAX_AVATAR_DATA_URL_LEN) {
            jpeg = canvas.toDataURL('image/jpeg', 0.3);
        }
        return { avatar: jpeg, isGif: false };
    }

    async function processGifAvatar(file) {
        const dataUrl = await readFileAsDataUrl(file);
        let avatar = dataUrl;
        let isGif = true;
        if (avatar.length > MAX_AVATAR_DATA_URL_LEN) {
            const resized = await resizeGifDataUrlToFit(dataUrl);
            avatar = resized.avatar;
            isGif = resized.isGif;
        }
        const staticFrame = await extractGifFirstFrame(avatar);
        return { avatar, staticFrame, isGif };
    }

    async function fitStaticDataUrl(dataUrl) {
        if (dataUrl.length <= MAX_AVATAR_DATA_URL_LEN) return dataUrl;
        const img = await loadImage(dataUrl);
        const maxDim = 400;
        const scale = Math.min(maxDim / img.naturalWidth, maxDim / img.naturalHeight, 1);
        const width = Math.max(1, Math.round(img.naturalWidth * scale));
        const height = Math.max(1, Math.round(img.naturalHeight * scale));
        const canvas = document.createElement('canvas');
        canvas.width = width;
        canvas.height = height;
        const ctx = canvas.getContext('2d');
        ctx.drawImage(img, 0, 0, width, height);
        for (const quality of [0.7, 0.5, 0.3]) {
            const out = canvas.toDataURL('image/jpeg', quality);
            if (out.length <= MAX_AVATAR_DATA_URL_LEN) return out;
        }
        return canvas.toDataURL('image/jpeg', 0.2);
    }

    globalThis.processGifAvatar = processGifAvatar;
    globalThis.fitStaticDataUrl = fitStaticDataUrl;
    // Test hooks (used by the node-based verification script).
    globalThis.__avatarTools = {
        encodeGifFrames,
        bytesToDataUrl,
        hashAndCheckAlpha,
        clampDelay,
        MAX_AVATAR_DATA_URL_LEN,
    };
})();
