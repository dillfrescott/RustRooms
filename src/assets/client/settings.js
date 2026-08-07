        const settingsOverlay = document.getElementById('settingsOverlay');
        const settingsNicknameInput = document.getElementById('settingsNicknameInput');
        const settingsAvatarInput = document.getElementById('settingsAvatarInput');
        const settingsAvatarPreview = document.getElementById('settingsAvatarPreview');
        const settingsAvatarPlaceholder = document.getElementById('settingsAvatarPlaceholder');
        let newAvatarCandidate = null;
        let newAvatarCandidateIsGif = false;
        let newAvatarCandidateStaticFrame = null;
        let settingsInitialAudioId = '';
        let settingsInitialVideoId = '';
        let settingsInitialAudioOutputId = '';
        let settingsNicknameDebounce = null;

        function handleSettingsNicknameInput() {
            userNickname = settingsNicknameInput.value.trim() || "Guest";
            savePreferences();
            updateLocalLabel();
            if (settingsNicknameDebounce) clearTimeout(settingsNicknameDebounce);
            settingsNicknameDebounce = setTimeout(() => {
                if (ws && ws.readyState === WebSocket.OPEN) {
                    ws.send(JSON.stringify({
                        type: "update-user",
                        data: { nickname: userNickname }
                    }));
                }
            }, 500);
        }

        async function handleSettingsMicChange(value) {
            currentAudioInputId = value;
            const currentVideoTrack = localStream ? localStream.getVideoTracks()[0] : null;
            const currentVideoId = currentVideoTrack ? currentVideoTrack.getSettings().deviceId : null;
            await switchMediaStream(value, currentVideoId);
            savePreferences();
        }

        async function handleSettingsCamChange(value) {
            currentVideoInputId = value;
            const currentAudioTrack = localStream ? localStream.getAudioTracks()[0] : null;
            const currentAudioId = currentAudioTrack ? currentAudioTrack.getSettings().deviceId : null;
            await switchMediaStream(currentAudioId, value);
            savePreferences();
        }

        async function openSettings() {
            settingsNicknameInput.value = userNickname;
            newAvatarCandidate = userAvatar;
            newAvatarCandidateIsGif = userAvatarIsGif;
            newAvatarCandidateStaticFrame = userAvatarStaticFrame;

            const settingsLBM = document.getElementById('settingsLowBandwidth');
            if (settingsLBM) settingsLBM.checked = isLowBandwidthMode;
            const settingsOtg = document.getElementById('settingsOnTheGo');
            if (settingsOtg) settingsOtg.checked = isOnTheGoMode;

            const removeBtn = document.getElementById('btnRemoveSettingsAvatar');
            if (userAvatar) {
                const displaySrc = userAvatarIsGif && userAvatarStaticFrame ? userAvatarStaticFrame : userAvatar;
                settingsAvatarPreview.src = displaySrc;
                settingsAvatarPreview.classList.remove('hidden');
                settingsAvatarPlaceholder.classList.add('hidden');
                if (removeBtn) removeBtn.classList.remove('hidden');
            } else {
                settingsAvatarPreview.classList.add('hidden');
                settingsAvatarPlaceholder.classList.remove('hidden');
                if (removeBtn) removeBtn.classList.add('hidden');
            }

            await populateSettingsDeviceList();
            const settingsAudio = document.getElementById('settingsAudioSource');
            const settingsVideo = document.getElementById('settingsVideoSource');
            const settingsAudioOutput = document.getElementById('settingsAudioOutputSource');
            settingsInitialAudioId = settingsAudio ? settingsAudio.value : '';
            settingsInitialVideoId = settingsVideo ? settingsVideo.value : '';
            settingsInitialAudioOutputId = settingsAudioOutput ? settingsAudioOutput.value : '';
            settingsOverlay.classList.remove('hidden');
            initSetupButtonTouchHandlers();
            if (localStream) {
                await setupVolumeMeter(localStream, 'settingsMicBar');
            }
        }

        function closeSettings() {
            settingsOverlay.classList.add('hidden');
            if (settingsMeterFrameId) cancelAnimationFrame(settingsMeterFrameId);
            if (isOnTheGoMode) {
                toggleOnTheGoMode(true, true);
            }
        }

        function handleSettingsAvatarUpload(input) {
            const file = input.files[0];
            if (!file) return;

            if (file.size > MAX_AVATAR_UPLOAD_SANITY_BYTES) {
                alert("File is too large! Maximum allowed size is 30MB.");
                input.value = '';
                return;
            }

            if (file.type === 'image/gif') {
                // Oversized GIFs are auto-resized browser-side (animation
                // preserved) to fit the server's avatar cap.
                processGifAvatar(file).then(result => {
                    newAvatarCandidate = result.avatar;
                    newAvatarCandidateIsGif = result.isGif;
                    newAvatarCandidateStaticFrame = result.staticFrame;
                    settingsAvatarPreview.src = result.staticFrame || result.avatar;
                    settingsAvatarPreview.classList.remove('hidden');
                    settingsAvatarPlaceholder.classList.add('hidden');
                    const removeBtn = document.getElementById('btnRemoveSettingsAvatar');
                    if (removeBtn) removeBtn.classList.remove('hidden');
                    saveSettings();
                }).catch(err => {
                    console.error("GIF avatar processing failed:", err);
                    showCustomAlert("Image Error", "Could not process this image. Please try a different file.");
                });
            } else {
                resizeImageForAvatar(file).then(dataUrl => {
                    openCropModal(dataUrl, 'settings');
                });
            }
            input.value = '';
        }

        async function saveSettings() {
            userAvatar = newAvatarCandidate;
            userAvatarIsGif = newAvatarCandidateIsGif;
            userAvatarStaticFrame = newAvatarCandidateStaticFrame;
            savePreferences();

            updateLocalAvatar();

            if (ws && ws.readyState === WebSocket.OPEN) {
                 ws.send(JSON.stringify({
                    type: "update-user",
                    data: {
                        nickname: userNickname,
                        avatar: userAvatar,
                        isGif: userAvatarIsGif,
                        staticFrame: userAvatarStaticFrame
                    }
                }));
            }

        }

        function updateLocalAvatar() {
             const layer = document.getElementById('localAvatarLayer');
             const img = document.getElementById('localAvatarImg');
             const centerImg = document.getElementById('localAvatarCenterImg');
             const placeholder = document.getElementById('localAvatarPlaceholder');

             let camEnabled = false;
             if (localStream) {
                 const videoTrack = localStream.getVideoTracks()[0];
                 if (videoTrack && videoTrack.enabled) camEnabled = true;
             }

             if (screenStream || camEnabled) {
                 if (screenStream) {
                     layer.style.display = 'none';
                 } else {
                    layer.style.display = 'none';
                 }
             } else {
                 layer.style.display = 'flex';
                 if (userAvatar) {
                     const displaySrc = userAvatarIsGif && userAvatarStaticFrame ? userAvatarStaticFrame : userAvatar;
                     img.src = displaySrc;
                     img.classList.remove('hidden');

                     centerImg.src = displaySrc;
                     centerImg.classList.remove('hidden');
                     placeholder.classList.add('hidden');
                 } else {
                     img.classList.add('hidden');
                     centerImg.classList.add('hidden');
                     placeholder.classList.remove('hidden');
                 }
             }
        }

        (function() {
            const pip = document.getElementById('localPipWrapper');
            const taskbar = document.querySelector('.taskbar');
            const connectionDot = document.getElementById('connectionDot');
            const btnCopy = document.getElementById('btnCopy');
            const sidebar = document.getElementById('roomSidebar');

            let isDragging = false;
            let dragOffset = { x: 0, y: 0 };
            let dragBounds = null;
            let pendingFrame = false;
            let collisionRects = null;
            let lastX = 0;
            let lastY = 0;

            function startDrag(clientX, clientY) {
                isDragging = true;
                pip.style.cursor = 'grabbing';
                pip.style.transition = 'none';

                const rect = pip.getBoundingClientRect();
                const taskbarRect = taskbar.getBoundingClientRect();
                const sidebarRect = sidebar && sidebar.classList.contains('open') ? sidebar.getBoundingClientRect() : null;

                pip.style.bottom = 'auto';
                pip.style.right = 'auto';
                pip.style.left = rect.left + 'px';
                pip.style.top = rect.top + 'px';

                dragOffset.x = clientX - rect.left;
                dragOffset.y = clientY - rect.top;

                lastX = clientX;
                lastY = clientY;

                let minX = 16;
                let maxX = window.innerWidth - rect.width - 16;
                if (sidebarRect) {
                    minX = sidebarRect.right + 16;
                }

                dragBounds = {
                    minX: minX,
                    maxX: maxX,
                    minY: 16,
                    maxY: window.innerHeight - taskbarRect.height - rect.height - 16
                };

                const margin = 16;
                collisionRects = {
                    statusRect: connectionDot && connectionDot.parentElement ? connectionDot.parentElement.getBoundingClientRect() : null,
                    copyRect: btnCopy ? btnCopy.getBoundingClientRect() : null,
                    sidebarRect: sidebarRect,
                    margin: margin,
                    pipWidth: rect.width
                };
            }

            function onMouseDown(e) {
                if (e.target.closest('button') || e.target.closest('input')) return;

                e.preventDefault();

                startDrag(e.clientX, e.clientY);
                document.addEventListener('mousemove', onMouseMove);
                document.addEventListener('mouseup', onMouseUp);
            }

            function onTouchStart(e) {
                if (e.target.closest('button') || e.target.closest('input')) return;

                const touch = e.touches[0];
                startDrag(touch.clientX, touch.clientY);

                document.addEventListener('touchmove', onTouchMove, { passive: false });
                document.addEventListener('touchend', onTouchEnd);
                document.addEventListener('touchcancel', onTouchEnd);
            }

            function handleMove(clientX, clientY) {
                lastX = clientX;
                lastY = clientY;

                if (!isDragging || pendingFrame) return;

                pendingFrame = true;

                requestAnimationFrame(() => {
                    if (!isDragging) {
                        pendingFrame = false;
                        return;
                    }

                    let newX = lastX - dragOffset.x;
                    let newY = lastY - dragOffset.y;

                    if (dragBounds) {
                        newX = Math.max(dragBounds.minX, Math.min(newX, dragBounds.maxX));
                        newY = Math.max(dragBounds.minY, Math.min(newY, dragBounds.maxY));
                    }

                    if (collisionRects) {
                        const { statusRect, copyRect, sidebarRect, margin, pipWidth } = collisionRects;

                        if (statusRect) {
                            const dangerRight = statusRect.right + margin;
                            const dangerBottom = statusRect.bottom + margin;

                            if (newX < dangerRight && newY < dangerBottom) {
                                const distToRight = dangerRight - newX;
                                const distToBottom = dangerBottom - newY;
                                if (distToRight < distToBottom) newX = dangerRight;
                                else newY = dangerBottom;
                            }
                        }

                        if (copyRect) {
                            const dangerLeft = copyRect.left - margin - pipWidth;
                            const dangerBottom = copyRect.bottom + margin;

                            if (newX > dangerLeft && newY < dangerBottom) {
                                const distToLeft = newX - dangerLeft;
                                const distToBottom = dangerBottom - newY;
                                if (distToLeft < distToBottom) newX = dangerLeft;
                                else newY = dangerBottom;
                            }
                        }

                        if (sidebarRect) {
                            const dangerRight = sidebarRect.right + margin;
                            const dangerBottom = sidebarRect.bottom + margin;

                            if (newX < dangerRight && newY < dangerBottom) {
                                const distToRight = dangerRight - newX;
                                const distToBottom = dangerBottom - newY;
                                if (distToRight < distToBottom) newX = dangerRight;
                                else newY = dangerBottom;
                            }
                        }
                    }

                    pip.style.left = newX + 'px';
                    pip.style.top = newY + 'px';
                    pendingFrame = false;
                });
            }

            function onMouseMove(e) {
                handleMove(e.clientX, e.clientY);
            }

            function onTouchMove(e) {
                if (e.cancelable) e.preventDefault();
                const touch = e.touches[0];
                handleMove(touch.clientX, touch.clientY);
            }

            function onMouseUp() {
                isDragging = false;
                pip.style.cursor = 'grab';
                pip.style.transition = '';
                document.removeEventListener('mousemove', onMouseMove);
                document.removeEventListener('mouseup', onMouseUp);
            }

            function onTouchEnd() {
                isDragging = false;
                pip.style.cursor = 'grab';
                pip.style.transition = '';
                document.removeEventListener('touchmove', onTouchMove);
                document.removeEventListener('touchend', onTouchEnd);
                document.removeEventListener('touchcancel', onTouchEnd);
            }

            pip.addEventListener('mousedown', onMouseDown);
            pip.addEventListener('touchstart', onTouchStart, { passive: false });

            let lastOrientation = window.innerWidth > window.innerHeight ? 'landscape' : 'portrait';
            let resizeTimeoutId = null;
            window.addEventListener('resize', () => {
                if (resizeTimeoutId) clearTimeout(resizeTimeoutId);

                resizeTimeoutId = setTimeout(() => {
                    const currentOrientation = window.innerWidth > window.innerHeight ? 'landscape' : 'portrait';
                    const isScreenFlip = currentOrientation !== lastOrientation;
                    lastOrientation = currentOrientation;

                    pip.style.left = '';
                    pip.style.top = '';
                    pip.style.bottom = '';
                    pip.style.right = '';

                    if (isScreenFlip) {
                        return;
                    }

                }, 250);
            });
        })();

        let idleTimer = null;
        document.addEventListener('mousemove', () => {
            if (document.fullscreenElement && document.fullscreenElement.classList.contains('video-container')) {
                document.fullscreenElement.classList.remove('idle-fullscreen');
                clearTimeout(idleTimer);
                idleTimer = setTimeout(() => {
                    if (document.fullscreenElement && document.fullscreenElement.classList.contains('video-container')) {
                        document.fullscreenElement.classList.add('idle-fullscreen');
                    }
                }, 2500);
            }
        });

        document.addEventListener('fullscreenchange', () => {
            if (!document.fullscreenElement) {
                clearTimeout(idleTimer);
                document.querySelectorAll('.video-container.idle-fullscreen').forEach(el => el.classList.remove('idle-fullscreen'));
            }
        });

        let currentCroppie = null;
        let currentCropTarget = null;

        function openCropModal(imageUrl, target) {
            currentCropTarget = target;
            const modal = document.getElementById('cropModal');
            const wrapper = document.getElementById('cropWrapper');
            wrapper.innerHTML = '';
            modal.classList.remove('hidden');

            currentCroppie = new Croppie(wrapper, {
                viewport: { width: 200, height: 200, type: 'square' },
                boundary: { width: '100%', height: 250 },
                showZoomer: true,
                enableOrientation: true
            });
            currentCroppie.bind({ url: imageUrl, zoom: 0 });
        }

        function closeCropModal() {
            document.getElementById('cropModal').classList.add('hidden');
            if (currentCroppie) {
                currentCroppie.destroy();
                currentCroppie = null;
            }
        }

        function applyCrop() {
            if (!currentCroppie) return;
            currentCroppie.result({
                type: 'base64',
                size: { width: 400, height: 400 },
                format: 'jpeg',
                quality: 0.8
            }).then(function(base64) {
                return fitStaticDataUrl(base64).then(function(fitBase64) {
                if (currentCropTarget === 'setup') {
                    userAvatar = fitBase64;
                    userAvatarIsGif = false;
                    userAvatarStaticFrame = null;
                    avatarPreview.src = userAvatar;
                    avatarPreview.classList.remove('hidden');
                    avatarPlaceholder.classList.add('hidden');
                    const removeBtn = document.getElementById('btnRemoveSetupAvatar');
                    if (removeBtn) removeBtn.classList.remove('hidden');
                    savePreferences();
                } else if (currentCropTarget === 'settings') {
                    newAvatarCandidate = fitBase64;
                    newAvatarCandidateIsGif = false;
                    newAvatarCandidateStaticFrame = null;
                    settingsAvatarPreview.src = newAvatarCandidate;
                    settingsAvatarPreview.classList.remove('hidden');
                    settingsAvatarPlaceholder.classList.add('hidden');
                    const removeBtn = document.getElementById('btnRemoveSettingsAvatar');
                    if (removeBtn) removeBtn.classList.remove('hidden');
                    closeCropModal();
                    saveSettings();
                    return;
                }
                closeCropModal();
                });
            });
        }
