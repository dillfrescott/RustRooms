/* ============================================================
 * ink.js — dynamic "ink drop / marbling" accent engine.
 *
 * Slowly swirls the app's blue accent through different shades
 * (azure → indigo → violet-blue) using layered sine waves with
 * random per-session phases, so the drift feels organic and
 * never repeats. All blue surfaces read the CSS variables this
 * updates (--accent, --accent-blue, --accent-glow, ...), which
 * includes primary buttons and the VAD speaking glow.
 * ============================================================ */
(function () {
    'use strict';

    const root = document.documentElement;
    const motionPreference = window.matchMedia('(prefers-reduced-motion: reduce)');

    // Random per-session phases — every visit marbles differently.
    const ph = [];
    for (let i = 0; i < 8; i++) ph.push(Math.random() * Math.PI * 2);

    // Layered slow sines: smooth pseudo-random wander in [-1, 1].
    function wander(t, f, phase) {
        return (
            Math.sin(t * f + phase) * 0.55 +
            Math.sin(t * f * 0.37 + phase * 1.7) * 0.3 +
            Math.sin(t * f * 1.61 + phase * 0.6) * 0.15
        );
    }

    function apply(t) {
        // Primary ink shade: hue drifts across the blue family.
        const hue  = 236 + wander(t, 0.050, ph[0]) * 26;   // ~210 – 262
        const sat  = 82  + wander(t, 0.034, ph[1]) * 12;
        const lig  = 61  + wander(t, 0.062, ph[2]) * 7;

        // Companion shade for gradients / glow highlights.
        const hue2 = hue + wander(t, 0.041, ph[3]) * 20;
        const lig2 = Math.min(82, lig + 13 + wander(t, 0.055, ph[4]) * 5);

        // Deep shade for marble edges.
        const hue3 = hue - wander(t, 0.027, ph[5]) * 14;
        const lig3 = Math.max(34, lig - 19 + wander(t, 0.047, ph[6]) * 4);

        root.style.setProperty('--accent',           `hsl(${hue.toFixed(1)} ${sat.toFixed(1)}% ${lig.toFixed(1)}%)`);
        root.style.setProperty('--accent-hover',     `hsl(${hue.toFixed(1)} ${sat.toFixed(1)}% ${(lig - 8).toFixed(1)}%)`);
        root.style.setProperty('--accent-blue',      `hsl(${hue2.toFixed(1)} ${sat.toFixed(1)}% ${lig2.toFixed(1)}%)`);
        root.style.setProperty('--accent-dark-blue', `hsl(${hue3.toFixed(1)} ${(sat - 6).toFixed(1)}% ${lig3.toFixed(1)}%)`);
        root.style.setProperty('--accent-glow',      `hsl(${hue.toFixed(1)} ${sat.toFixed(1)}% ${lig.toFixed(1)}% / 0.38)`);
        root.style.setProperty('--border-accent',    `hsl(${hue.toFixed(1)} ${sat.toFixed(1)}% ${lig.toFixed(1)}%)`);
    }

    let last = 0;
    let animationId = null;
    const t0 = performance.now() + Math.random() * 60000; // random start point in the flow

    function frame(now) {
        if (motionPreference.matches) {
            animationId = null;
            apply(0);
            return;
        }

        // ~24 fps is plenty for a slow swirl and keeps CPU idle-low.
        if (now - last > 42) {
            last = now;
            apply((now - t0) / 1000);
        }
        animationId = requestAnimationFrame(frame);
    }

    function updateMotionPreference() {
        if (motionPreference.matches) {
            if (animationId !== null) cancelAnimationFrame(animationId);
            animationId = null;
            apply(0);
        } else if (animationId === null) {
            last = 0;
            animationId = requestAnimationFrame(frame);
        }
    }

    if (motionPreference.addEventListener) {
        motionPreference.addEventListener('change', updateMotionPreference);
    } else {
        motionPreference.addListener(updateMotionPreference);
    }
    updateMotionPreference();
})();
