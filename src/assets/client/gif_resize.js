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
    // Capture dim for cropped avatars: cropping happens in capture space, so
    // a higher dim keeps zoomed-in crops from getting blurry. Bounded so a
    // pathological GIF can't blow up memory (2.5 MB per frame at 800px).
    const GIF_CROP_CAPTURE_DIM = 800;
    const GIF_MAX_FRAMES = 150;
    const GIF_QUIET_MS = 2000;
    // If the very first frame holds for GIF_QUIET_MS the capture would
    // conclude the GIF is static before the animation has shown a single
    // change (long intro cards, slow-start memes). Only apply the quiet
    // timeout after at least one frame change was observed; for a frame
    // that has never changed, wait up to this longer cap before treating
    // the GIF as truly static.
    const GIF_STATIC_MAX_MS = 8000;
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

    function loadImageDims(dataUrl) {
        return new Promise((resolve, reject) => {
            const img = new Image();
            img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight });
            img.onerror = () => reject(new Error('Failed to decode image'));
            img.src = dataUrl;
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
    // (static tail / long hold; the first frame gets GIF_STATIC_MAX_MS so
    // slow-starting animations aren't mistaken for static), or at the frame
    // cap.
    function captureGifFramesRaf(dataUrl, maxDim) {
        return new Promise((resolve, reject) => {
            const img = new Image();
            // The browser only runs a GIF's animation while it is painted;
            // attach the image (invisibly) so frames actually advance.
            img.style.position = 'fixed';
            img.style.left = '0';
            img.style.top = '0';
            img.style.width = '1px';
            img.style.height = '1px';
            img.style.opacity = '0.01';
            img.style.pointerEvents = 'none';
            img.style.zIndex = '-1';
            img.alt = '';
            if (document.body) {
                document.body.appendChild(img);
            }
            const cleanup = () => {
                if (img.parentNode) {
                    img.parentNode.removeChild(img);
                }
            };
            img.onerror = () => {
                cleanup();
                reject(new Error('Failed to decode GIF'));
            };
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
                    cleanup();
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
                        const hold = now - lastChange;
                        if (
                            (frames.length > 1 && hold >= GIF_QUIET_MS) ||
                            (frames.length === 1 && hold >= GIF_STATIC_MAX_MS) ||
                            frames.length >= GIF_MAX_FRAMES
                        ) {
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

    // Converts a base64 data URL to a Uint8Array. Must not use fetch():
    // the app's CSP (connect-src 'self' wss: ws:) blocks data: URLs.
    function dataUrlToBytes(dataUrl) {
        const comma = dataUrl.indexOf(',');
        if (comma < 0) {
            throw new Error('Not a data URL');
        }
        const binary = atob(dataUrl.slice(comma + 1));
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) {
            bytes[i] = binary.charCodeAt(i);
        }
        return bytes;
    }

    // Decodes every frame of a GIF via WebCodecs ImageDecoder (Chrome) into
    // canvases (maxDim-capped) plus their GCE delays. Works with zero
    // dependence on browser GIF playback. Returns null where ImageDecoder
    // is unavailable.
    async function decodeGifFrames(dataUrl, maxDim) {
        if (typeof ImageDecoder === 'undefined') {
            return null;
        }
        if (!(await ImageDecoder.isTypeSupported('image/gif'))) {
            return null;
        }
        const decoder = new ImageDecoder({ data: dataUrlToBytes(dataUrl), type: 'image/gif' });
        try {
            await decoder.tracks.ready;
            const track = decoder.tracks.selectedTrack;
            const known = track ? track.frameCount : 0;
            // GIF has no frame count field; Chrome may report Infinity for
            // large files, so iterate until decode fails.
            const total = typeof known === 'number' && Number.isFinite(known) && known > 0
                ? Math.min(known, GIF_MAX_FRAMES)
                : GIF_MAX_FRAMES;
            const frames = [];
            let width = 0;
            let height = 0;
            for (let i = 0; i < total; i++) {
                let image = null;
                try {
                    const res = await decoder.decode({ frameIndex: i });
                    image = res.image;
                } catch (err) {
                    if (frames.length === 0) {
                        throw err;
                    }
                    break;
                }
                if (!image) {
                    if (frames.length === 0) {
                        throw new Error('No frames captured');
                    }
                    break;
                }
                try {
                    if (frames.length === 0) {
                        const scale = Math.min(maxDim / image.displayWidth, maxDim / image.displayHeight, 1);
                        width = Math.max(1, Math.round(image.displayWidth * scale));
                        height = Math.max(1, Math.round(image.displayHeight * scale));
                    }
                    // VideoFrame.duration is in microseconds; the last frame
                    // often reports null, so reuse the previous delay.
                    let delay = frames.length > 0 ? frames[frames.length - 1].delay : 100;
                    if (image.duration != null) {
                        delay = Math.max(0, image.duration / 1000);
                    }
                    // Draw to a canvas instead of createImageBitmap: VideoFrame
                    // is directly drawable and this avoids any bitmap API
                    // availability surprises.
                    const canvas = document.createElement('canvas');
                    canvas.width = width;
                    canvas.height = height;
                    const cctx = canvas.getContext('2d');
                    cctx.drawImage(image, 0, 0, width, height);
                    frames.push({ canvas, delay });
                } finally {
                    image.close();
                }
                await yieldToEventLoop();
            }
            if (frames.length === 0) {
                throw new Error('No frames captured');
            }
            return { frames, width, height };
        } finally {
            decoder.close();
        }
    }

    // Deterministic capture via WebCodecs ImageDecoder (Chrome): decodes
    // every GIF frame on demand with its exact GCE delay, independent of
    // browser GIF playback. Falls back to canvas capture elsewhere.
    async function captureGifFramesWithDecoder(dataUrl, maxDim) {
        const decoded = await decodeGifFrames(dataUrl, maxDim);
        const canvas = document.createElement('canvas');
        canvas.width = decoded.width;
        canvas.height = decoded.height;
        const ctx = canvas.getContext('2d', { willReadFrequently: true });
        const frames = [];
        const hashes = [];
        let hasAlpha = false;
        for (const frame of decoded.frames) {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            ctx.drawImage(frame.canvas, 0, 0, canvas.width, canvas.height);
            const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
            const result = hashAndCheckAlpha(data);
            hasAlpha = hasAlpha || result.hasAlpha;
            const dup = hashes.indexOf(result.hash);
            if (dup >= 0) {
                // Identical to an earlier frame: merge the display time back
                // (same loop structure the canvas capture produces) so
                // repeated frames don't bloat the file.
                frames[dup].delay += frame.delay;
            } else {
                frames.push({ data: new Uint8ClampedArray(data), delay: frame.delay });
                hashes.push(result.hash);
            }
            await yieldToEventLoop();
        }
        return { frames, width: decoded.width, height: decoded.height, hasAlpha };
    }

    async function captureGifFrames(dataUrl, maxDim) {
        if (typeof ImageDecoder !== 'undefined') {
            try {
                if (await ImageDecoder.isTypeSupported('image/gif')) {
                    return await captureGifFramesWithDecoder(dataUrl, maxDim);
                }
            } catch (err) {
                // Fall back to canvas capture on any decoder failure, but
                // surface it so issues are visible in the console.
                console.warn('GIF decoder capture failed, using canvas capture:', err);
            }
        }
        return captureGifFramesRaf(dataUrl, maxDim);
    }

    // Yields to the event loop so WebSocket heartbeat timers keep firing
    // during long processing; the server closes connections that stay
    // silent for 10s ("Inactivity timeout").
    function yieldToEventLoop() {
        return new Promise((resolve) => setTimeout(resolve, 0));
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

    async function encodeGifFrames(frames, width, height, hasAlpha) {
        const enc = globalThis.__gifenc;
        const gif = new enc.GIFEncoder();
        for (const frame of frames) {
            // GIF transparency is binary, so semi-transparent pixels were
            // never actually blended by GIF decoders; encoding in rgb565 and
            // forcing fully transparent pixels to a dedicated transparent
            // palette entry is visually equivalent to rgba4444 but ~20x
            // faster (rgba4444 alpha-aware distance is very slow).
            const rgba = new Uint8ClampedArray(frame.data);
            // gifenc's quantize is O(distinct-colors^2) and explodes on
            // photo-like frames (~3s/frame); rounding colors first caps the
            // distinct-color count and makes it fast (~10-250ms/frame).
            // roundRGB must divide 255 (e.g. 15) or values near 255 wrap
            // around to black in gifenc's bit packing.
            enc.prequantize(rgba, { roundRGB: 15, roundAlpha: 255 });
            let palette;
            let index;
            let transparentIndex = -1;
            if (hasAlpha) {
                palette = enc.quantize(rgba, 255, { format: 'rgb565' });
                palette.push([0, 0, 1, 0]);
                transparentIndex = palette.length - 1;
                index = enc.applyPalette(rgba, palette, 'rgb565');
                for (let i = 0; i < frame.data.length; i += 4) {
                    if (frame.data[i + 3] === 0) {
                        index[i >> 2] = transparentIndex;
                    }
                }
            } else {
                palette = enc.quantize(rgba, 256, { format: 'rgb565' });
                index = enc.applyPalette(rgba, palette, 'rgb565');
            }
            const opts = { palette, delay: clampDelay(frame.delay), repeat: 0 };
            if (transparentIndex >= 0) {
                opts.transparent = true;
                opts.transparentIndex = transparentIndex;
            }
            gif.writeFrame(index, width, height, opts);
            await yieldToEventLoop();
        }
        gif.finish();
        return bytesToDataUrl(gif.bytes(), 'image/gif');
    }

    async function downscaleGifFrames(frames, srcW, srcH, dstW, dstH) {
        const src = document.createElement('canvas');
        src.width = srcW;
        src.height = srcH;
        const sctx = src.getContext('2d');
        const dst = document.createElement('canvas');
        dst.width = dstW;
        dst.height = dstH;
        const dctx = dst.getContext('2d');
        const out = [];
        for (const frame of frames) {
            sctx.putImageData(new ImageData(frame.data, srcW, srcH), 0, 0);
            dctx.clearRect(0, 0, dstW, dstH);
            dctx.drawImage(src, 0, 0, dstW, dstH);
            out.push({
                data: new Uint8ClampedArray(dctx.getImageData(0, 0, dstW, dstH).data),
                delay: frame.delay,
            });
            await yieldToEventLoop();
        }
        return out;
    }

    async function resizeGifDataUrlToFit(dataUrl) {
        const captured = await captureGifFrames(dataUrl, GIF_MAX_CAPTURE_DIM);
        // Rough raw-byte budget a data URL of MAX_AVATAR_DATA_URL_LEN fits
        // (base64 is 4/3 of raw). Indexed GIF frames run ~0.8 bytes/pixel
        // after LZW for typical avatar content; skip dims whose total pixel
        // data can't fit so we don't burn an encode pass on them.
        const rawBudget = Math.floor((MAX_AVATAR_DATA_URL_LEN * 3) / 4);
        const dims = [400, 320, 256, 200, 160, 128];
        for (const dim of dims) {
            if (dim > captured.width && dim > captured.height) continue;
            const scale = Math.min(dim / captured.width, dim / captured.height, 1);
            const width = Math.max(1, Math.round(captured.width * scale));
            const height = Math.max(1, Math.round(captured.height * scale));
            if (captured.frames.length * width * height * 0.8 > rawBudget) continue;
            const frames = (width === captured.width && height === captured.height)
                ? captured.frames
                : await downscaleGifFrames(captured.frames, captured.width, captured.height, width, height);
            const out = await encodeGifFrames(frames, width, height, captured.hasAlpha);
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

    function cropGifFrames(frames, srcW, srcH, cx, cy, cw, ch) {
        const src = document.createElement('canvas');
        src.width = srcW;
        src.height = srcH;
        const sctx = src.getContext('2d');
        const dst = document.createElement('canvas');
        dst.width = cw;
        dst.height = ch;
        const dctx = dst.getContext('2d');
        const out = [];
        for (const frame of frames) {
            sctx.clearRect(0, 0, srcW, srcH);
            sctx.putImageData(new ImageData(frame.data, srcW, srcH), 0, 0);
            dctx.clearRect(0, 0, cw, ch);
            dctx.drawImage(src, cx, cy, cw, ch, 0, 0, cw, ch);
            out.push({
                data: new Uint8ClampedArray(dctx.getImageData(0, 0, cw, ch).data),
                delay: frame.delay,
            });
            yieldToEventLoop();
        }
        return out;
    }

    // Crops every frame of an animated GIF to the given rectangle and
    // re-encodes it, preserving the animation. cropRect is in the original
    // image's pixel space ({ x, y, width, height }, as returned by
    // Croppie.get()). Downscales to fit the server cap if needed.
    async function cropGifDataUrl(dataUrl, cropRect) {
        const natural = await loadImageDims(dataUrl);
        const captured = await captureGifFrames(dataUrl, GIF_CROP_CAPTURE_DIM);
        const scale = captured.width / natural.width;
        let cx = Math.round(cropRect.x * scale);
        let cy = Math.round(cropRect.y * scale);
        let cw = Math.max(1, Math.round(cropRect.width * scale));
        let ch = Math.max(1, Math.round(cropRect.height * scale));
        cx = Math.max(0, Math.min(cx, captured.width - 1));
        cy = Math.max(0, Math.min(cy, captured.height - 1));
        cw = Math.max(1, Math.min(cw, captured.width - cx));
        ch = Math.max(1, Math.min(ch, captured.height - cy));
        const cropped = cropGifFrames(captured.frames, captured.width, captured.height, cx, cy, cw, ch);
        const rawBudget = Math.floor((MAX_AVATAR_DATA_URL_LEN * 3) / 4);
        const maxSide = Math.max(cw, ch);
        const sizes = [{ w: cw, h: ch }];
        for (const dim of [400, 320, 256, 200, 160, 128]) {
            if (dim < maxSide) {
                const s = dim / maxSide;
                sizes.push({ w: Math.max(1, Math.round(cw * s)), h: Math.max(1, Math.round(ch * s)) });
            }
        }
        let last = null;
        for (const size of sizes) {
            if (last && last.w === size.w && last.h === size.h) continue;
            last = size;
            if (cropped.length * size.w * size.h * 0.8 > rawBudget) continue;
            const frames = (size.w === cw && size.h === ch)
                ? cropped
                : await downscaleGifFrames(cropped, cw, ch, size.w, size.h);
            const avatar = await encodeGifFrames(frames, size.w, size.h, captured.hasAlpha);
            if (avatar.length <= MAX_AVATAR_DATA_URL_LEN) {
                const staticFrame = await extractGifFirstFrame(avatar);
                return { avatar, staticFrame, isGif: true };
            }
        }
        // Practically unreachable fallback: first frame as a small JPEG.
        const canvas = document.createElement('canvas');
        canvas.width = cw;
        canvas.height = ch;
        const ctx = canvas.getContext('2d');
        ctx.putImageData(new ImageData(cropped[0].data, cw, ch), 0, 0);
        let jpeg = canvas.toDataURL('image/jpeg', 0.7);
        if (jpeg.length > MAX_AVATAR_DATA_URL_LEN) {
            jpeg = canvas.toDataURL('image/jpeg', 0.3);
        }
        const staticFrame = await extractGifFirstFrame(jpeg);
        return { avatar: jpeg, staticFrame, isGif: false };
    }

    // JS-driven GIF player for the speaking animation. Some browsers
    // (enterprise AnimationPolicy, extensions) never advance GIF frames in
    // <img> elements; stepping pre-decoded frames with a timer still works
    // there. Returns null when ImageDecoder is unavailable (browsers that
    // animate GIFs natively keep using the plain <img> src swap).
    // maxDim caps the frame capture size (larger = sharper previews when the
    // GIF is displayed larger than GIF_MAX_CAPTURE_DIM).
    async function createGifAnimator(dataUrl, maxDim) {
        const decoded = await decodeGifFrames(dataUrl, maxDim || GIF_MAX_CAPTURE_DIM);
        if (!decoded || decoded.frames.length < 2) {
            return null;
        }
        const frames = decoded.frames;
        let timer = null;
        let index = 0;
        let onFrame = null;
        const tick = () => {
            if (onFrame) {
                onFrame(frames[index].canvas);
            }
            const delay = Math.max(MIN_FRAME_DELAY_MS, Math.min(MAX_FRAME_DELAY_MS, frames[index].delay));
            index = (index + 1) % frames.length;
            timer = setTimeout(tick, delay);
        };
        return {
            start(callback) {
                onFrame = callback;
                index = 0;
                tick();
            },
            stop() {
                if (timer) {
                    clearTimeout(timer);
                    timer = null;
                }
                onFrame = null;
            },
        };
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
    globalThis.createGifAnimator = createGifAnimator;
    globalThis.readAvatarFile = readFileAsDataUrl;
    globalThis.cropGifAvatar = cropGifDataUrl;
    // Test hooks (used by the diagnostic page).
    globalThis.__captureGifFramesWithDecoder = captureGifFramesWithDecoder;
    // Test hooks (used by the node-based verification script).
    globalThis.__avatarTools = {
        encodeGifFrames,
        bytesToDataUrl,
        hashAndCheckAlpha,
        clampDelay,
        MAX_AVATAR_DATA_URL_LEN,
    };
})();
