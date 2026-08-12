(function () {
    'use strict';

    const motionPreference = window.matchMedia('(prefers-reduced-motion: reduce)');

    function setupParticleCanvas(canvasId, overlayId) {
        const canvas = document.getElementById(canvasId);
        const overlay = document.getElementById(overlayId);
        if (!canvas || !overlay) return;

        const ctx = canvas.getContext('2d');
        let particles = [];
        let animationId = null;

        class Particle {
            constructor() {
                this.x = Math.random() * canvas.width;
                this.y = Math.random() * canvas.height;
                this.vx = (Math.random() - 0.5) * 0.5;
                this.vy = (Math.random() - 0.5) * 0.5;
                this.radius = Math.random() * 2 + 1;
                this.opacity = Math.random() * 0.5 + 0.2;
                this.color = Math.random() > 0.5 ? '129, 140, 248' : '99, 102, 241';
            }

            update() {
                this.x += this.vx;
                this.y += this.vy;
                if (this.x < 0) this.x = canvas.width;
                if (this.x > canvas.width) this.x = 0;
                if (this.y < 0) this.y = canvas.height;
                if (this.y > canvas.height) this.y = 0;
            }

            draw() {
                ctx.beginPath();
                ctx.arc(this.x, this.y, this.radius, 0, Math.PI * 2);
                ctx.fillStyle = `rgba(${this.color}, ${this.opacity})`;
                ctx.fill();
            }
        }

        function resize() {
            canvas.width = window.innerWidth;
            canvas.height = window.innerHeight;
            particles = [];
        }

        function init() {
            particles = [];
            const particleCount = Math.floor((canvas.width * canvas.height) / 18000);
            for (let i = 0; i < particleCount; i++) {
                particles.push(new Particle());
            }
        }

        function drawParticles(shouldMove) {
            if (particles.length === 0) init();
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            particles.forEach((particle) => {
                if (shouldMove) particle.update();
                particle.draw();
            });
        }

        function stopAnimation() {
            if (animationId !== null) {
                cancelAnimationFrame(animationId);
                animationId = null;
            }
        }

        function animate() {
            if (motionPreference.matches || document.hidden) {
                animationId = null;
                drawParticles(false);
                return;
            }
            drawParticles(true);
            animationId = requestAnimationFrame(animate);
        }

        function isOverlayVisible() {
            const style = window.getComputedStyle(overlay);
            return style.display !== 'none' && style.visibility !== 'hidden' && !document.hidden;
        }

        function checkVisibility() {
            if (!isOverlayVisible()) {
                stopAnimation();
                particles = [];
                ctx.clearRect(0, 0, canvas.width, canvas.height);
                return;
            }

            if (motionPreference.matches) {
                stopAnimation();
                drawParticles(false);
            } else if (animationId === null) {
                animate();
            }
        }

        resize();
        checkVisibility();

        window.addEventListener('resize', () => {
            resize();
            checkVisibility();
        });

        const observer = new MutationObserver(checkVisibility);
        observer.observe(overlay, { attributes: true, attributeFilter: ['style', 'class'] });
        document.addEventListener('visibilitychange', checkVisibility);

        if (motionPreference.addEventListener) {
            motionPreference.addEventListener('change', checkVisibility);
        } else {
            motionPreference.addListener(checkVisibility);
        }
    }

    setupParticleCanvas('particleCanvas', 'welcomeOverlay');
    setupParticleCanvas('particleCanvasConfig', 'configOverlay');
    setupParticleCanvas('particleCanvasInvite', 'inviteWelcomeOverlay');
})();
