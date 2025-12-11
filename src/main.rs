mod turn_server;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

fn get_html_page(turn_user: &str, turn_pass: &str) -> String {
    let html = r###"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no, viewport-fit=cover">
    <title>Rust Rooms</title>
    <link rel="icon" type="image/png" href="https://i.imgur.com/S7Ham4d.png">
    <script src="https://cdn.tailwindcss.com"></script>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet">
    <style>
        html {
            height: 100%;
            overflow: hidden;
            overscroll-behavior: none;
        }

        body { 
            background-color: #0f172a; 
            color: #f8fafc; 
            font-family: 'Inter', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; 
            overflow: hidden; 
            position: fixed;
            inset: 0;
            width: 100%;
            height: 100dvh;
            touch-action: pan-x pan-y;
        }
        
        ::-webkit-scrollbar { width: 8px; }
        ::-webkit-scrollbar-track { background: #1e293b; }
        ::-webkit-scrollbar-thumb { background: #475569; border-radius: 4px; }
        ::-webkit-scrollbar-thumb:hover { background: #64748b; }

        .glass-panel {
            background: rgba(30, 41, 59, 0.95);
            backdrop-filter: blur(12px);
            -webkit-backdrop-filter: blur(12px);
            border: 1px solid rgba(255, 255, 255, 0.1);
        }

        .video-container {
            position: relative;
            background: #1e293b;
            border-radius: 1rem;
            overflow: hidden;
            box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
            transition: all 0.3s ease;
            display: flex;
            flex-direction: column;
            width: 100%;
            height: 100%;
        }
        
        .video-container video {
            width: 100%;
            height: 100%;
            object-fit: contain;
            background: transparent;
        }

        .grid-expand {
            grid-auto-rows: minmax(0, 1fr);
        }

        .avatar-layer {
            position: absolute;
            inset: 0;
            display: flex;
            align-items: center;
            justify-content: center;
            background: #1e293b;
            z-index: 10;
        }

        .avatar-img {
            position: absolute;
            inset: 0;
            width: 100%;
            height: 100%;
            object-fit: cover;
            filter: blur(20px); 
            opacity: 0.4;
        }

        .avatar-center {
            position: relative;
            width: 80px; 
            height: 80px;
            border-radius: 50%;
            overflow: hidden;
            border: 3px solid rgba(255, 255, 255, 0.1);
            background: #334155;
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 20;
        }
        
        @media (min-width: 768px) {
            .avatar-center {
                width: 120px;
                height: 120px;
                border-width: 4px;
            }
        }
        
        .avatar-center img {
            width: 100%;
            height: 100%;
            object-fit: cover;
        }
        
        video.active + .avatar-layer {
            display: none !important;
        }

        .control-btn {
            padding: 12px;
            border-radius: 12px;
            border: none;
            cursor: pointer;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
            background: rgba(51, 65, 85, 0.5);
            color: white;
            width: 48px;
            height: 48px;
        }
        
        @media (min-width: 768px) {
            .control-btn {
                width: 56px;
                height: 56px;
            }
        }
        
        .control-btn:hover {
            background: rgba(71, 85, 105, 1);
            transform: translateY(-2px);
        }

        .control-btn.active-red {
            background: #ef4444;
            box-shadow: 0 0 15px rgba(239, 68, 68, 0.4);
        }
        
        .control-btn.active-green {
            background: #22c55e;
            box-shadow: 0 0 15px rgba(34, 197, 94, 0.4);
        }

        .pip-wrapper {
            position: fixed;
            bottom: 220px; 
            right: 16px;
            cursor: grab;
            touch-action: none;
            width: 120px;
            aspect-ratio: 16/9;
            border-radius: 0.75rem;
            border: 2px solid rgba(255, 255, 255, 0.1);
            overflow: hidden;
            z-index: 40;
            box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1);
            transition: transform 0.2s;
            background: #1e293b;
        }
        
        @media (min-width: 768px) {
            .pip-wrapper {
                width: 280px;
                bottom: 240px;
            }
        }
        
        .pip-wrapper:hover {
            transform: scale(1.02);
        }

        .connection-dot {
            width: 8px;
            height: 8px;
            background-color: #ef4444;
            border-radius: 50%;
            display: inline-block;
            margin-right: 8px;
        }
        .connection-dot.connected { background-color: #22c55e; box-shadow: 0 0 8px #22c55e; }
        .connection-dot.connecting { background-color: #f59e0b; animation: pulse 2s infinite; }

        input[type=range] {
            -webkit-appearance: none; 
            background: transparent; 
        }
        input[type=range]::-webkit-slider-thumb {
            -webkit-appearance: none;
            height: 12px;
            width: 12px;
            border-radius: 50%;
            background: #ffffff;
            cursor: pointer;
            margin-top: -4px; 
            box-shadow: 0 0 2px rgba(0,0,0,0.5);
        }
        input[type=range]::-webkit-slider-runnable-track {
            width: 100%;
            height: 4px;
            cursor: pointer;
            background: rgba(255,255,255,0.3);
            border-radius: 2px;
        }
        
        .volume-controls {
            position: absolute;
            bottom: 8px;
            right: 8px;
            background: rgba(0, 0, 0, 0.6);
            backdrop-filter: blur(4px);
            padding: 6px 10px;
            border-radius: 12px;
            display: flex;
            flex-direction: column;
            gap: 4px;
            opacity: 0;
            transition: opacity 0.2s;
            align-items: flex-end;
        }
        .video-container:hover .volume-controls {
            opacity: 1;
        }
        
        .vol-row {
            display: flex;
            align-items: center;
            gap: 8px;
        }

        .speaking-glow {
            box-shadow: 0 0 0 4px #22c55e !important;
            transition: box-shadow 0.1s ease-in-out;
        }

        .video-container:fullscreen {
            border-radius: 0;
            background: #000;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        
        .video-container:fullscreen video {
            max-height: 100vh;
            max-width: 100vw;
            height: 100%;
            width: 100%;
        }

        .video-container:fullscreen .volume-controls {
            bottom: 32px;
            right: 32px;
            transform: scale(1.2);
            transform-origin: bottom right;
            padding: 12px;
            gap: 8px;
        }

        /* Taskbar style footer */
        .taskbar {
            background: rgba(15, 23, 42, 0.95);
            border-top: 1px solid rgba(255, 255, 255, 0.1);
            backdrop-filter: blur(10px);
            padding-bottom: env(safe-area-inset-bottom);
        }

        @media (min-width: 768px) {
            .taskbar {
                padding-bottom: env(safe-area-inset-bottom);
            }
        }

    </style>
</head>
<body class="flex flex-col overflow-hidden bg-slate-900">

    <div id="welcomeOverlay" class="fixed inset-0 z-[70] bg-slate-900 flex flex-col items-center justify-center p-4 transition-opacity duration-300" style="display: none;">
        <div class="text-center space-y-6 max-w-md w-full">
            <h1 class="text-4xl md:text-5xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-blue-400 to-emerald-400">Rust Rooms</h1>
            <p class="text-slate-400 text-base md:text-lg">Simple, secure, and fast video conferencing.</p>
            <button onclick="createRoom()" class="w-full md:w-auto px-8 py-4 bg-blue-600 hover:bg-blue-500 text-white rounded-full font-bold text-lg shadow-lg shadow-blue-500/30 transition-all transform hover:scale-105">
                Start Room
            </button>
        </div>
    </div>

    <div id="configOverlay" class="fixed inset-0 z-[60] bg-slate-900 flex flex-col items-center justify-center p-4 transition-opacity duration-300 hidden opacity-0">
        <div class="glass-panel p-6 md:p-8 rounded-2xl max-w-lg w-full shadow-2xl space-y-6 border border-slate-700 max-h-[90vh] overflow-y-auto">
            <div class="text-center space-y-2">
                <h1 class="text-2xl md:text-3xl font-bold text-white">Setup</h1>
                <p class="text-slate-400 text-sm">Configure your stream.</p>
            </div>

            <div class="relative aspect-video bg-slate-800 rounded-xl overflow-hidden border border-slate-600 shadow-inner">
                <video id="previewVideo" autoplay playsinline muted class="w-full h-full object-contain"></video>
                <div class="absolute inset-0 flex items-center justify-center text-slate-500 pointer-events-none" id="previewPlaceholder">
                    <span>Camera Off</span>
                </div>
                <div class="absolute bottom-3 left-3 bg-black/50 px-2 py-1 rounded text-xs text-white backdrop-blur-sm">
                    Preview
                </div>
            </div>

            <div class="space-y-4">
                <div class="flex flex-col md:flex-row gap-4">
                    <div class="flex-shrink-0 flex md:block justify-center">
                         <div class="text-center">
                            <label class="block text-xs font-medium text-slate-400 mb-1">Avatar</label>
                            <div onclick="document.getElementById('avatarInput').click()" class="w-16 h-16 rounded-full bg-slate-700 border-2 border-slate-600 hover:border-blue-500 cursor-pointer overflow-hidden flex items-center justify-center transition-colors group relative mx-auto">
                                <img id="avatarPreview" src="" class="hidden w-full h-full object-cover">
                                <div id="avatarPlaceholder" class="text-2xl">👤</div>
                                <div class="absolute inset-0 bg-black/50 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity text-xs font-bold">Edit</div>
                            </div>
                            <input type="file" id="avatarInput" hidden accept="image/*" onchange="handleAvatarUpload(this)">
                        </div>
                    </div>

                    <div class="flex-1 space-y-3">
                        <div>
                            <label class="block text-xs font-medium text-slate-400 mb-1">Nickname</label>
                            <input type="text" id="nicknameInput" placeholder="Enter your name" class="w-full bg-slate-800 border border-slate-600 rounded-lg px-4 py-2 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-all" maxlength="32">
                        </div>
                        
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                             <div>
                                <label class="block text-xs font-medium text-slate-400 mb-1">Microphone</label>
                                <select id="audioSource" onchange="startPreview()" class="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:ring-2 focus:ring-blue-500">
                                    <option value="">Default</option>
                                </select>
                            </div>
                            <div>
                                <label class="block text-xs font-medium text-slate-400 mb-1">Speaker</label>
                                <select id="audioOutputSource" onchange="changeAudioOutput(this.value)" class="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:ring-2 focus:ring-blue-500">
                                    <option value="default">Default</option>
                                </select>
                            </div>
                            <div>
                                <label class="block text-xs font-medium text-slate-400 mb-1">Camera</label>
                                <select id="videoSource" onchange="startPreview()" class="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:ring-2 focus:ring-blue-500">
                                    <option value="">Default</option>
                                </select>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="pt-2 flex gap-3">
                <button onclick="togglePreviewMic()" id="btnPreviewMic" class="flex-1 py-3 bg-slate-700 hover:bg-slate-600 text-white rounded-lg font-medium transition-colors flex items-center justify-center gap-2">
                    Mute
                </button>
                 <button onclick="togglePreviewCam()" id="btnPreviewCam" class="flex-1 py-3 bg-slate-700 hover:bg-slate-600 text-white rounded-lg font-medium transition-colors flex items-center justify-center gap-2">
                    Stop Cam
                </button>
            </div>

            <button onclick="joinRoom()" class="w-full py-3 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-bold shadow-lg shadow-blue-500/30 transition-all transform hover:scale-[1.02]">
                Join Room
            </button>
        </div>
    </div>

    <div id="settingsOverlay" class="fixed inset-0 z-[80] bg-black/80 flex items-center justify-center p-4 hidden">
        <div class="glass-panel p-6 rounded-2xl max-w-md w-full shadow-2xl space-y-6 border border-slate-700 relative max-h-[90vh] overflow-y-auto">
             <button onclick="closeSettings()" class="absolute top-4 right-4 text-slate-400 hover:text-white">
                <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
            </button>
            
            <h2 class="text-2xl font-bold text-white">Settings</h2>
            
            <div class="space-y-4">
                <div class="flex flex-col items-center gap-2">
                    <label class="block text-xs font-medium text-slate-400">Avatar</label>
                    <div onclick="document.getElementById('settingsAvatarInput').click()" class="w-24 h-24 rounded-full bg-slate-700 border-2 border-slate-600 hover:border-blue-500 cursor-pointer overflow-hidden flex items-center justify-center transition-colors group relative">
                        <img id="settingsAvatarPreview" src="" class="hidden w-full h-full object-cover">
                        <div id="settingsAvatarPlaceholder" class="text-4xl">👤</div>
                         <div class="absolute inset-0 bg-black/50 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity text-xs font-bold">Change</div>
                    </div>
                    <input type="file" id="settingsAvatarInput" hidden accept="image/*" onchange="handleSettingsAvatarUpload(this)">
                </div>

                <div>
                    <label class="block text-xs font-medium text-slate-400 mb-1">Nickname</label>
                    <input type="text" id="settingsNicknameInput" placeholder="Enter your name" class="w-full bg-slate-800 border border-slate-600 rounded-lg px-4 py-2 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-all" maxlength="32">
                </div>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                     <div>
                        <label class="block text-xs font-medium text-slate-400 mb-1">Microphone</label>
                        <select id="settingsAudioSource" class="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:ring-2 focus:ring-blue-500">
                        </select>
                    </div>
                     <div>
                        <label class="block text-xs font-medium text-slate-400 mb-1">Speaker</label>
                        <select id="settingsAudioOutputSource" class="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:ring-2 focus:ring-blue-500">
                        </select>
                    </div>
                    <div>
                        <label class="block text-xs font-medium text-slate-400 mb-1">Camera</label>
                        <select id="settingsVideoSource" class="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:ring-2 focus:ring-blue-500">
                        </select>
                    </div>
                </div>
            </div>

            <div class="pt-2">
                <button onclick="saveSettings()" class="w-full py-3 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-bold shadow-lg shadow-blue-500/30 transition-all">
                    Save Changes
                </button>
            </div>
        </div>
    </div>

    <div id="appLayout" class="hidden flex-col h-full w-full">
        <div class="flex-none p-3 md:p-4 z-40 flex justify-between items-center">
            <div class="glass-panel px-3 py-1.5 md:px-4 md:py-2 rounded-full flex items-center gap-2">
                <div id="connectionDot" class="connection-dot"></div>
                <span id="statusText" class="text-xs md:text-sm font-medium text-slate-200">Waiting...</span>
                <button id="btnReconnect" onclick="retryConnection()" class="hidden ml-2 p-1.5 rounded-full hover:bg-slate-700 text-slate-400 hover:text-white transition-all" title="Retry Connection">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/></svg>
                </button>
            </div>

            <div id="btnCopy" class="glass-panel px-3 py-1.5 md:px-4 md:py-2 rounded-full cursor-pointer hover:bg-slate-700/50 transition-all flex items-center gap-2" onclick="copyLink()">
                <span class="text-xs md:text-sm font-medium text-slate-200">Invite Link</span>
                <svg id="iconCopy" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>
            </div>
        </div>

        <main class="flex-1 w-full relative min-h-0">
            <div class="absolute inset-0 p-2 md:p-4 overflow-hidden flex items-center justify-center">
                 <div id="remoteGrid" class="grid gap-4 w-full h-full max-w-[1600px] transition-all duration-500 grid-expand"></div>
            </div>
            
            <div id="emptyState" class="hidden absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 text-center text-slate-500 pointer-events-none">
                <div class="mb-4">
                    <svg class="mx-auto h-16 w-16 opacity-50" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                    </svg>
                </div>
                <p class="text-xl font-semibold">Waiting for others to join...</p>
                <p class="text-sm mt-2">Share the invite link to get started.</p>
            </div>

            <div class="pip-wrapper" id="localPipWrapper">
                 <div class="w-full h-full relative flex flex-col">
                    <div id="localAvatarLayer" class="absolute inset-0 z-20 bg-slate-800 flex items-center justify-center" style="display: none;">
                        <img id="localAvatarImg" src="" class="absolute inset-0 w-full h-full object-cover filter blur-xl opacity-40 hidden">
                        <div class="relative w-12 h-12 md:w-20 md:h-20 rounded-full bg-slate-700 border-2 border-slate-600 flex items-center justify-center overflow-hidden z-10">
                             <img id="localAvatarCenterImg" src="" class="w-full h-full object-cover hidden">
                             <div id="localAvatarPlaceholder" class="text-xl md:text-3xl">👤</div>
                        </div>
                    </div>
                    
                    <video id="localVideo" autoplay playsinline muted class="w-full h-full object-cover z-10"></video>
                    <div id="localLabel" class="absolute bottom-2 left-2 bg-black/50 px-2 py-1 rounded text-[10px] md:text-xs text-white backdrop-blur-sm z-30">
                        You
                    </div>
                </div>
            </div>
        </main>

        <footer class="flex-none taskbar w-full z-50">
            <div class="flex justify-center items-center py-3 md:py-4 gap-3 md:gap-4 px-4">
                <button class="control-btn" id="btnMic" onclick="toggleMic()" title="Toggle Microphone">
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" x2="12" y1="19" y2="22"/></svg>
                </button>
                <button class="control-btn" id="btnCam" onclick="toggleCam()" title="Toggle Camera">
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z"/><circle cx="12" cy="13" r="3"/></svg>
                </button>
                <button class="control-btn hover:text-blue-400" id="btnShare" onclick="toggleScreen()" title="Share Screen">
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/></svg>
                </button>
                <button class="control-btn hover:text-blue-400" onclick="openSettings()" title="Settings">
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
                </button>
                <div class="w-px bg-slate-600 mx-1"></div>
                <button class="control-btn active-red" onclick="location.href='/'" title="Leave Room">
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" x2="9" y1="12" y2="12"/></svg>
                </button>
            </div>
        </footer>
    </div>

    <script>
        const roomId = window.location.pathname.split('/')[2];
        const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${wsProtocol}//${window.location.host}/ws/${roomId}`;
        
        let ws;
        let localStream;
        let screenStream;
        let peers = {}; 
        let peerCamStatus = {};
        let peerScreenStatus = {};
        let userNickname = "Guest";
        let userAvatar = null;
        let isConfigured = false;
        let audioContext;
        let wakeLock = null;
        let currentAudioOutputId = 'default';
        
        let reconnectionAttempts = 0;
        const maxReconnectionAttempts = 5;
        const reconnectionDelay = 3000;
        
        const rtcConfig = {
            iceServers: [
                { urls: `stun:${window.location.hostname}:3478` },
                {
                    urls: `turn:${window.location.hostname}:3478`,
                    username: "{{TURN_USER}}", 
                    credential: "{{TURN_PASS}}" 
                }
            ]
        };

        const localVideo = document.getElementById('localVideo');
        const previewVideo = document.getElementById('previewVideo');
        const remoteGrid = document.getElementById('remoteGrid');
        const emptyState = document.getElementById('emptyState');
        const connectionDot = document.getElementById('connectionDot');
        const statusText = document.getElementById('statusText');
        const configOverlay = document.getElementById('configOverlay');
        const appLayout = document.getElementById('appLayout');
        const nicknameInput = document.getElementById('nicknameInput');
        const audioSelect = document.getElementById('audioSource');
        const audioOutputSelect = document.getElementById('audioOutputSource');
        const videoSelect = document.getElementById('videoSource');
        const avatarPreview = document.getElementById('avatarPreview');
        const avatarPlaceholder = document.getElementById('avatarPlaceholder');
        
        function getPersistentId() {
            let id = sessionStorage.getItem('room_user_id');
            if (!id) {
                id = Math.random().toString(36).substring(2) + Date.now().toString(36);
                sessionStorage.setItem('room_user_id', id);
            }
            return id;
        }

        async function requestWakeLock() {
            try {
                if ('wakeLock' in navigator) {
                    wakeLock = await navigator.wakeLock.request('screen');
                    wakeLock.addEventListener('release', () => {
                        console.log('Wake Lock released');
                    });
                    console.log('Wake Lock active');
                }
            } catch (err) {
                console.error(`${err.name}, ${err.message}`);
            }
        }
        
        document.addEventListener('visibilitychange', async () => {
            if (wakeLock !== null && document.visibilityState === 'visible') {
                await requestWakeLock();
            }
        });

        async function loadDevices() {
            loadPreferences();
            try {
                localStream = await navigator.mediaDevices.getUserMedia({ audio: true, video: true });
                previewVideo.srcObject = localStream;
                document.getElementById('previewPlaceholder').style.display = 'none';
                updatePreviewButtons();
                await new Promise(r => setTimeout(r, 500));
                await populateDeviceList();
                navigator.mediaDevices.ondevicechange = populateDeviceList;

            } catch (e) {
                console.warn("Device access failed", e);
                try {
                    localStream = await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
                    updatePreviewButtons();
                    await populateDeviceList();
                } catch(e2) {
                     console.error("Audio failed too", e2);
                     updatePreviewButtons();
                }
            }
        }

        async function populateDeviceList() {
            try {
                const devices = await navigator.mediaDevices.enumerateDevices();
                const currentAudio = audioSelect.value;
                const currentAudioOutput = currentAudioOutputId;
                const currentVideo = videoSelect.value;
                
                const audioTrack = localStream ? localStream.getAudioTracks()[0] : null;
                const videoTrack = localStream ? localStream.getVideoTracks()[0] : null;
                
                const activeAudioId = audioTrack ? audioTrack.getSettings().deviceId : null;
                const activeVideoId = videoTrack ? videoTrack.getSettings().deviceId : null;

                audioSelect.innerHTML = '';
                audioOutputSelect.innerHTML = '';
                videoSelect.innerHTML = '';
                
                devices.forEach(device => {
                    const option = document.createElement('option');
                    option.value = device.deviceId;
                    option.text = device.label || `${device.kind} (${device.deviceId.slice(0,5)}...)`;
                    if (device.kind === 'audioinput') {
                        audioSelect.appendChild(option);
                    } else if (device.kind === 'audiooutput') {
                        audioOutputSelect.appendChild(option);
                    }
                    else if (device.kind === 'videoinput') videoSelect.appendChild(option);
                });

                if (activeAudioId && [...audioSelect.options].some(o => o.value === activeAudioId)) {
                    audioSelect.value = activeAudioId;
                }

                if (currentAudioOutput && [...audioOutputSelect.options].some(o => o.value === currentAudioOutput)) {
                    audioOutputSelect.value = currentAudioOutput;
                }
                
                if (activeVideoId && [...videoSelect.options].some(o => o.value === activeVideoId)) {
                    videoSelect.value = activeVideoId;
                }

            } catch(e) {
                console.error("Enumeration error", e);
            }
        }
        
        async function populateSettingsDeviceList() {
            try {
                const devices = await navigator.mediaDevices.enumerateDevices();
                const settingsAudio = document.getElementById('settingsAudioSource');
                const settingsAudioOutput = document.getElementById('settingsAudioOutputSource');
                const settingsVideo = document.getElementById('settingsVideoSource');
                
                const audioTrack = localStream ? localStream.getAudioTracks()[0] : null;
                const videoTrack = localStream ? localStream.getVideoTracks()[0] : null;
                
                const activeAudioId = audioTrack ? audioTrack.getSettings().deviceId : null;
                const activeAudioOutputId = currentAudioOutputId;
                const activeVideoId = videoTrack ? videoTrack.getSettings().deviceId : null;

                settingsAudio.innerHTML = '';
                settingsAudioOutput.innerHTML = '';
                settingsVideo.innerHTML = '';
                
                devices.forEach(device => {
                    const option = document.createElement('option');
                    option.value = device.deviceId;
                    option.text = device.label || `${device.kind} (${device.deviceId.slice(0,5)}...)`;
                    if (device.kind === 'audioinput') {
                        settingsAudio.appendChild(option);
                    } else if (device.kind === 'audiooutput') {
                        settingsAudioOutput.appendChild(option);
                    }
                    else if (device.kind === 'videoinput') settingsVideo.appendChild(option);
                });
                
                 if (activeAudioId && [...settingsAudio.options].some(o => o.value === activeAudioId)) {
                    settingsAudio.value = activeAudioId;
                }

                if (activeAudioOutputId && [...settingsAudioOutput.options].some(o => o.value === activeAudioOutputId)) {
                    settingsAudioOutput.value = activeAudioOutputId;
                }
                
                if (activeVideoId && [...settingsVideo.options].some(o => o.value === activeVideoId)) {
                    settingsVideo.value = activeVideoId;
                }
            } catch (e) { console.error(e); }
        }

        async function changeAudioOutput(deviceId) {
            currentAudioOutputId = deviceId;
            const elements = document.querySelectorAll('video, audio');
            for (const el of elements) {
                await attachSinkId(el, deviceId);
            }
            savePreferences();
        }

        async function attachSinkId(element, sinkId) {
            if (typeof element.setSinkId === 'function') {
                try {
                    await element.setSinkId(sinkId);
                } catch (e) {
                    console.warn("Failed to set audio output device", e);
                }
            }
        }

        async function switchMediaStream(audioId, videoId) {
             const currentAudioTrack = localStream ? localStream.getAudioTracks()[0] : null;
             const currentVideoTrack = localStream ? localStream.getVideoTracks()[0] : null;
             const currentAudioId = currentAudioTrack ? currentAudioTrack.getSettings().deviceId : "";
             const currentVideoId = currentVideoTrack ? currentVideoTrack.getSettings().deviceId : "";
             
             if (audioId && audioId !== currentAudioId) {
                 try {
                     const constraints = { audio: { deviceId: { exact: audioId } } };
                     const stream = await navigator.mediaDevices.getUserMedia(constraints);
                     const newTrack = stream.getAudioTracks()[0];
                     
                     if (currentAudioTrack) {
                         currentAudioTrack.stop();
                         localStream.removeTrack(currentAudioTrack);
                     }
                     localStream.addTrack(newTrack);
                     
                     for (const userId in peers) {
                        const pc = peers[userId];
                        const sender = pc.getSenders().find(s => s.track && s.track.kind === 'audio');
                        if (sender) {
                             sender.replaceTrack(newTrack);
                        } else {
                             pc.addTrack(newTrack, localStream);
                             negotiate(userId, pc);
                        }
                     }
                     
                     setupAudioMonitor(localStream, 'local');
                     
                 } catch (e) {
                     console.error("Audio switch failed", e);
                     alert("Failed to switch microphone: " + e.message);
                 }
             }
             
             if (videoId && videoId !== currentVideoId) {
                 try {
                     if (currentVideoTrack) {
                         currentVideoTrack.stop();
                         localStream.removeTrack(currentVideoTrack);
                     }
                     
                     await new Promise(r => setTimeout(r, 200));

                     const constraints = { video: { deviceId: { exact: videoId } } };
                     const stream = await navigator.mediaDevices.getUserMedia(constraints);
                     const newTrack = stream.getVideoTracks()[0];
                     
                     localStream.addTrack(newTrack);
                     
                     if (!screenStream) {
                        for (const userId in peers) {
                           const pc = peers[userId];
                           const sender = pc.getSenders().find(s => s.track && s.track.kind === 'video');
                           if (sender) {
                               sender.replaceTrack(newTrack);
                           } else {
                               pc.addTrack(newTrack, localStream);
                               negotiate(userId, pc);
                           }
                        }
                        
                        if (ws && ws.readyState === WebSocket.OPEN) {
                            ws.send(JSON.stringify({
                                type: 'cam-toggle',
                                data: { enabled: true }
                            }));
                        }
                     }

                 } catch (e) {
                     console.error("Video switch failed", e);
                 }
             }
             
             updateLocalAvatar();
        }

        function setupAudioMonitor(stream, targetId) {
            if (!audioContext) return;
            if (!stream.getAudioTracks().length) return;
            
            if (audioContext.state === 'suspended') {
                audioContext.resume();
            }

            const source = audioContext.createMediaStreamSource(stream);
            const analyser = audioContext.createAnalyser();
            analyser.fftSize = 256;
            source.connect(analyser);
            
            const bufferLength = analyser.frequencyBinCount;
            const dataArray = new Uint8Array(bufferLength);
            
            function checkAudio() {
                if (targetId !== 'local' && !document.getElementById(targetId)) return;
                
                analyser.getByteFrequencyData(dataArray);
                let sum = 0;
                for(let i = 0; i < bufferLength; i++) {
                    sum += dataArray[i];
                }
                const average = sum / bufferLength;
                
                let targetEl;
                if (targetId === 'local') {
                    targetEl = document.getElementById('localAvatarCenterImg')?.parentElement;
                } else {
                    const wrapper = document.getElementById(targetId);
                    if (wrapper) targetEl = wrapper.querySelector('.avatar-center');
                }
                
                if (targetEl) {
                    if (average > 10) { 
                        targetEl.classList.add('speaking-glow');
                    } else {
                        targetEl.classList.remove('speaking-glow');
                    }
                }
                
                requestAnimationFrame(checkAudio);
            }
            checkAudio();
        }

        function loadPreferences() {
            const stored = localStorage.getItem('iroh_profile');
            if (stored) {
                try {
                    const data = JSON.parse(stored);
                    if (data.nickname) nicknameInput.value = data.nickname;
                    if (data.avatar) {
                        userAvatar = data.avatar;
                        avatarPreview.src = userAvatar;
                        avatarPreview.classList.remove('hidden');
                        avatarPlaceholder.classList.add('hidden');
                    }
                    if (data.audioOutputId) {
                        currentAudioOutputId = data.audioOutputId;
                    }
                } catch (e) { console.error("Load pref error", e); }
            }
        }

        function savePreferences() {
            localStorage.setItem('iroh_profile', JSON.stringify({
                nickname: userNickname,
                avatar: userAvatar,
                audioOutputId: currentAudioOutputId
            }));
        }

        function handleAvatarUpload(input) {
            const file = input.files[0];
            if (!file) return;

            const reader = new FileReader();
            reader.onload = function(e) {
                const img = new Image();
                img.onload = function() {
                    const canvas = document.createElement('canvas');
                    const ctx = canvas.getContext('2d');
                    
                    const MAX_SIZE = 128;
                    let width = img.width;
                    let height = img.height;
                    
                    if (width > height) {
                        if (width > MAX_SIZE) {
                            height *= MAX_SIZE / width;
                            width = MAX_SIZE;
                        }
                    } else {
                        if (height > MAX_SIZE) {
                            width *= MAX_SIZE / height;
                            height = MAX_SIZE;
                        }
                    }
                    
                    canvas.width = width;
                    canvas.height = height;
                    ctx.drawImage(img, 0, 0, width, height);
                    
                    userAvatar = canvas.toDataURL('image/jpeg', 0.8);
                    avatarPreview.src = userAvatar;
                    avatarPreview.classList.remove('hidden');
                    avatarPlaceholder.classList.add('hidden');
                };
                img.src = e.target.result;
            };
            reader.readAsDataURL(file);
        }

        async function startPreview() {
            if (localStream) {
                localStream.getTracks().forEach(track => track.stop());
                localStream = null;
            }

            const audioSource = audioSelect.value;
            const videoSource = videoSelect.value;
            
            const constraints = {
                audio: { deviceId: audioSource ? { exact: audioSource } : undefined },
                video: { deviceId: videoSource ? { exact: videoSource } : undefined }
            };

            try {
                localStream = await navigator.mediaDevices.getUserMedia(constraints);
                previewVideo.srcObject = localStream;
                document.getElementById('previewPlaceholder').style.display = 'none';
                updatePreviewButtons();
            } catch (e) {
                console.error("Preview failed", e);
                document.getElementById('previewPlaceholder').style.display = 'flex';
                 try {
                    localStream = await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
                    previewVideo.srcObject = null;
                    updatePreviewButtons();
                } catch(e2) {
                    updatePreviewButtons();
                }
            }
        }

        function updatePreviewButtons() {
             const btnMic = document.getElementById('btnPreviewMic');
             const btnCam = document.getElementById('btnPreviewCam');

             if (!localStream) {
                 btnMic.disabled = true;
                 btnMic.classList.add('opacity-50', 'cursor-not-allowed');
                 btnMic.innerText = "No Mic";
                 
                 btnCam.disabled = true;
                 btnCam.classList.add('opacity-50', 'cursor-not-allowed');
                 btnCam.innerText = "No Cam";
                 document.getElementById('previewPlaceholder').style.display = 'flex';
                 return;
             }
             
             const audioTrack = localStream.getAudioTracks()[0];
             const videoTrack = localStream.getVideoTracks()[0];

             if (!audioTrack) {
                 btnMic.disabled = true;
                 btnMic.classList.add('opacity-50', 'cursor-not-allowed');
                 btnMic.innerText = "No Mic";
             } else {
                 btnMic.disabled = false;
                 btnMic.classList.remove('opacity-50', 'cursor-not-allowed');
                 if (!audioTrack.enabled) {
                     btnMic.classList.add('bg-red-500', 'hover:bg-red-600');
                     btnMic.innerText = "Unmute";
                 } else {
                     btnMic.classList.remove('bg-red-500', 'hover:bg-red-600');
                     btnMic.innerText = "Mute";
                 }
             }

             if (!videoTrack) {
                 btnCam.disabled = true;
                 btnCam.classList.add('opacity-50', 'cursor-not-allowed');
                 btnCam.innerText = "No Cam";
                 document.getElementById('previewPlaceholder').style.display = 'flex';
             } else {
                 btnCam.disabled = false;
                 btnCam.classList.remove('opacity-50', 'cursor-not-allowed');
                 if (!videoTrack.enabled) {
                     btnCam.classList.add('bg-red-500', 'hover:bg-red-600');
                     btnCam.innerText = "Start Cam";
                     document.getElementById('previewPlaceholder').style.display = 'flex';
                 } else {
                     btnCam.classList.remove('bg-red-500', 'hover:bg-red-600');
                     btnCam.innerText = "Stop Cam";
                     document.getElementById('previewPlaceholder').style.display = 'none';
                 }
             }
        }

        function togglePreviewMic() {
            if (!localStream) return;
            const track = localStream.getAudioTracks()[0];
            if (track) {
                track.enabled = !track.enabled;
                updatePreviewButtons();
            }
        }

        function togglePreviewCam() {
             if (!localStream) return;
            const track = localStream.getVideoTracks()[0];
            if (track) {
                track.enabled = !track.enabled;
                updatePreviewButtons();
            }
        }

        async function joinRoom() {
            userNickname = nicknameInput.value.trim() || "Guest";
            savePreferences();
            
            if (!audioContext) {
                audioContext = new (window.AudioContext || window.webkitAudioContext)();
            }
            if (audioContext.state === 'suspended') {
                await audioContext.resume();
            }

            previewVideo.srcObject = null;
            
            configOverlay.classList.add('opacity-0', 'pointer-events-none');
            setTimeout(() => {
                configOverlay.style.display = 'none';
                appLayout.classList.remove('hidden');
                appLayout.classList.add('flex');
            }, 300);

            localVideo.srcObject = localStream;
            
            updateLocalLabel();
            updateLocalAvatar();
            const btnMic = document.getElementById('btnMic');
            const btnCam = document.getElementById('btnCam');

            const micOffSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="1" y1="1" x2="23" y2="23"></line><path d="M9 9v3a3 3 0 0 0 5.12 2.12M15 9.34V4a3 3 0 0 0-5.94-.6"></path><path d="M17 16.95A7 7 0 0 1 5 12v-2m14 0v2a7 7 0 0 1-.11 1.23"></path><line x1="12" x2="12" y1="19" y2="22"></line></svg>`;
            const camOffSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="1" y1="1" x2="23" y2="23"></line><path d="M21 21l-3.5-3.5m-2-2l-2-2m-2-2l-2-2m-2-2l-3.5-3.5"></path><path d="M15 7h5a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-5"></path><path d="M4 8v8a2 2 0 0 0 2 2h4.5"></path></svg>`;
            
             if (localStream) {
                const audioTrack = localStream.getAudioTracks()[0];
                const videoTrack = localStream.getVideoTracks()[0];
                
                if (!audioTrack || !audioTrack.enabled) {
                     btnMic.classList.add('active-red');
                     btnMic.innerHTML = micOffSvg;
                }
                if (!videoTrack || !videoTrack.enabled) {
                     btnCam.classList.add('active-red');
                     btnCam.innerHTML = camOffSvg;
                }
                
                setupAudioMonitor(localStream, 'local');
            } else {
                 btnMic.classList.add('active-red');
                 btnMic.innerHTML = micOffSvg;
                 btnCam.classList.add('active-red');
                 btnCam.innerHTML = camOffSvg;
            }

            connectWs();
            await requestWakeLock();
        }

        const welcomeOverlay = document.getElementById('welcomeOverlay');

        function updateStatus(state, message) {
            statusText.innerText = message;
            connectionDot.className = 'connection-dot ' + state;
        }

        function createRoom() {
            window.location.href = '/new';
        }

        if (roomId) {
            configOverlay.classList.remove('hidden');
            configOverlay.classList.remove('opacity-0');
            loadDevices();
        } else {
            welcomeOverlay.style.display = 'flex';
        }

        function connectWs() {
            updateStatus('connecting', 'Connecting...');
            ws = new WebSocket(wsUrl);
            
                        ws.onopen = () => {
                            reconnectionAttempts = 0;
                            updateStatus('connected', 'Connected');
                            const camEnabled = localStream && localStream.getVideoTracks()[0] && localStream.getVideoTracks()[0].enabled;
                            const screenEnabled = !!screenStream;
                            const screenHasAudio = screenStream && screenStream.getAudioTracks().length > 0;
                            const myId = getPersistentId();
                            ws.send(JSON.stringify({
                                type: "join", 
                                userId: myId,
                                data: {
                                    nickname: userNickname,
                                    avatar: userAvatar,
                                    camEnabled: camEnabled,
                                    screenEnabled: screenEnabled,
                                    screenAudio: screenHasAudio
                                }
                            }));
                            checkEmpty();
                        };
            
                        ws.onmessage = async (event) => {
                            const msg = JSON.parse(event.data);
                            
                            switch (msg.type) {
                                case 'user-joined':
                                    if (peers[msg.userId]) {
                                        removePeer(msg.userId);
                                    }
            
                                    if (msg.data.camEnabled !== undefined) {
                                        peerCamStatus[msg.userId] = msg.data.camEnabled;
                                    }
                                    if (msg.data.screenEnabled !== undefined) {
                                        peerScreenStatus[msg.userId] = msg.data.screenEnabled;
                                    }
                                    initPeer(msg.userId, true, msg.data?.nickname, msg.data?.avatar);
                                    
                                    const myCamEnabled = localStream && localStream.getVideoTracks()[0] && localStream.getVideoTracks()[0].enabled;
                                    const myScreenEnabled = !!screenStream;
                                    const myScreenHasAudio = screenStream && screenStream.getAudioTracks().length > 0;
                                    ws.send(JSON.stringify({
                                        type: 'identify',
                                        target: msg.userId,
                                        data: {
                                            nickname: userNickname, 
                                            avatar: userAvatar,
                                            camEnabled: myCamEnabled,
                                            screenEnabled: myScreenEnabled,
                                            screenAudio: myScreenHasAudio
                                        }
                                    }));
                                    break;
                                case 'user-left':
                                    removePeer(msg.userId);
                                    delete peerCamStatus[msg.userId];
                                    delete peerScreenStatus[msg.userId];
                                    break;
                                case 'user-update':
                                     updatePeerInfo(msg.userId, msg.data.nickname, msg.data.avatar);
                                    break;
                                case 'cam-toggle':
                                    if (msg.data && msg.data.enabled !== undefined) {
                                        peerCamStatus[msg.userId] = msg.data.enabled;
                                    }
                                    break;
                                case 'screen-toggle':
                                    if (msg.data && msg.data.enabled !== undefined) {
                                        peerScreenStatus[msg.userId] = msg.data.enabled;
                                        const v = document.getElementById(`vid-${msg.userId}`);
                                        if (v) v.style.objectFit = msg.data.enabled ? 'contain' : 'contain';
            
                                        if (!msg.data.enabled || !msg.data.hasAudio) {
                                            const row = document.getElementById(`vol-row-screen-${msg.userId}`);
                                            if (row) row.remove();
                                            const aud = document.getElementById(`aud-screen-${msg.userId}`);
                                            if (aud) aud.remove();
                                        }
                                    }
                                    break;
                                case 'identify':
                                    if (msg.data.camEnabled !== undefined) {
                                        peerCamStatus[msg.userId] = msg.data.camEnabled;
                                    }
                                    if (msg.data.screenEnabled !== undefined) {
                                        peerScreenStatus[msg.userId] = msg.data.screenEnabled;
                                    }
                                    if (peers[msg.userId]) {
                                        updatePeerInfo(msg.userId, msg.data.nickname, msg.data.avatar);
                                    } else {
                                        initPeer(msg.userId, false, msg.data.nickname, msg.data.avatar);
                                    }
                                    break;
                                case 'signal':
                                    handleSignal(msg.userId, msg.data);
                                    break;
                            }
                        };
                        
                        ws.onclose = () => {
                            reconnectionAttempts++;
                            if (reconnectionAttempts >= maxReconnectionAttempts) {
                                updateStatus('disconnected', 'Disconnected');
                                const btn = document.getElementById('btnReconnect');
                                if (btn) btn.classList.remove('hidden');
                                console.error('WebSocket disconnected after multiple retries. No further attempts will be made.');
                            } else {
                                updateStatus('connecting', `Reconnecting... (Attempt ${reconnectionAttempts}/${maxReconnectionAttempts})`);
                                setTimeout(connectWs, reconnectionDelay);
                            }
                        };
            
                        ws.onerror = (error) => {
                            console.error('WebSocket Error:', error);
                        };
                    }

        function retryConnection() {
            const btn = document.getElementById('btnReconnect');
            if (btn) {
                btn.classList.add('text-green-500', 'bg-green-500/10');
                btn.classList.remove('text-slate-400', 'hover:text-white', 'hover:bg-slate-700');
                
                setTimeout(() => {
                    btn.classList.add('hidden');
                    btn.classList.remove('text-green-500', 'bg-green-500/10');
                    btn.classList.add('text-slate-400', 'hover:text-white', 'hover:bg-slate-700');
                    
                    reconnectionAttempts = 0;
                    connectWs();
                }, 300);
            }
        }
        
        function setAvatar(layer, avatar) {
            layer.innerHTML = '';
            if (avatar) {
               const bgImg = document.createElement('img');
               bgImg.src = avatar;
               bgImg.className = 'avatar-img';
               
               const centerDiv = document.createElement('div');
               centerDiv.className = 'avatar-center';
               
               const centerImg = document.createElement('img');
               centerImg.src = avatar;
               
               centerDiv.appendChild(centerImg);
               layer.appendChild(bgImg);
               layer.appendChild(centerDiv);
           } else {
               const centerDiv = document.createElement('div');
               centerDiv.className = 'avatar-center';
               centerDiv.style.background = 'transparent';
               centerDiv.style.border = 'none';
               
               const text = document.createElement('div');
               text.className = 'text-6xl mb-2';
               text.innerText = '👤';
               
               centerDiv.appendChild(text);
               layer.appendChild(centerDiv);
           }
        }

        function updatePeerInfo(userId, nickname, avatar) {
            const wrapper = document.getElementById(`wrapper-${userId}`);
            if (wrapper) {
                const label = wrapper.querySelector('.absolute.bottom-3.left-3');
                if (label) label.innerText = nickname || "Unknown";
                
                const avatarLayer = wrapper.querySelector('.avatar-layer');
                if (avatarLayer) {
                     setAvatar(avatarLayer, avatar);
                }
            }
        }

        function checkEmpty() {
            const count = Object.keys(peers).length;
            if (count === 0) {
                emptyState.style.display = 'block';
            } else {
                emptyState.style.display = 'none';
            }
            updateGridLayout(count);
        }
        
        function updateGridLayout(count) {
            remoteGrid.className = 'grid gap-2 md:gap-4 w-full h-full max-w-[1600px] transition-all duration-500 grid-expand';
            
            if (count === 0) return;
            if (count === 1) remoteGrid.classList.add('grid-cols-1');
            else if (count === 2) remoteGrid.classList.add('grid-cols-1', 'md:grid-cols-2');
            else if (count <= 4) remoteGrid.classList.add('grid-cols-2');
            else if (count <= 6) remoteGrid.classList.add('grid-cols-2', 'md:grid-cols-3');
            else if (count <= 9) remoteGrid.classList.add('grid-cols-3');
            else remoteGrid.classList.add('grid-cols-4');
        }

        function forceStereoAudio(sdp) {
            let sdpLines = sdp.split('\r\n');
            let opusPayload = -1;
            let rtpmapLineIndex = -1;

            for (let i = 0; i < sdpLines.length; i++) {
                if (sdpLines[i].startsWith('a=rtpmap:')) {
                    if (sdpLines[i].includes('opus/48000')) {
                        opusPayload = sdpLines[i].split(':')[1].split(' ')[0];
                        rtpmapLineIndex = i;
                        break;
                    }
                }
            }

            if (opusPayload === -1) return sdp;
            
            let fmtpLineIndex = -1;
            for (let i = 0; i < sdpLines.length; i++) {
                if (sdpLines[i].startsWith('a=fmtp:' + opusPayload)) {
                    fmtpLineIndex = i;
                    break;
                }
            }
            
            if (fmtpLineIndex === -1) {
                sdpLines.splice(rtpmapLineIndex + 1, 0, 'a=fmtp:' + opusPayload + ' stereo=1;sprop-stereo=1;maxaveragebitrate=510000;useinbandfec=1;cbr=1;usedtx=0');
            } else {
                let fmtpLine = sdpLines[fmtpLineIndex];
                if (!fmtpLine.includes('stereo=1')) {
                    sdpLines[fmtpLineIndex] = fmtpLine + ';stereo=1;sprop-stereo=1;maxaveragebitrate=510000;useinbandfec=1;cbr=1;usedtx=0';
                }
            }
            return sdpLines.join('\r\n');
        }

        function negotiate(userId, pc) {
            pc.createOffer()
                .then(offer => {
                    offer.sdp = forceStereoAudio(offer.sdp);
                    return pc.setLocalDescription(offer);
                })
                .then(() => sendSignal(userId, { type: 'offer', sdp: pc.localDescription }))
                .catch(e => console.error("Negotiation error", e));
        }

        function initPeer(userId, initiator, nickname, avatarUrl) {
            if (peers[userId]) return; 
            
            const displayName = nickname || `User ${userId.substr(0,4)}`;

            const pc = new RTCPeerConnection(rtcConfig);
            peers[userId] = pc;

            if (localStream) {
                localStream.getAudioTracks().forEach(track => pc.addTrack(track, localStream));
            }

            if (screenStream) {
                const screenTrack = screenStream.getVideoTracks()[0];
                if (screenTrack) {
                    if (localStream) {
                        pc.addTrack(screenTrack, localStream);
                    } else {
                        pc.addTrack(screenTrack, screenStream);
                    }
                }
                const screenAudioTrack = screenStream.getAudioTracks()[0];
                if (screenAudioTrack) {
                    const sender = pc.addTrack(screenAudioTrack, screenStream);
                    const params = sender.getParameters();
                    if (!params.encodings) params.encodings = [{}];
                    params.encodings[0].maxBitrate = 512000;
                    sender.setParameters(params).catch(e => console.warn(e));
                }
            } else if (localStream) {
                localStream.getVideoTracks().forEach(track => pc.addTrack(track, localStream));
            }

            if (!localStream || localStream.getVideoTracks().length === 0) {
                 pc.addTransceiver('video', { direction: 'recvonly' });
            }
            if (!localStream || localStream.getAudioTracks().length === 0) {
                 pc.addTransceiver('audio', { direction: 'recvonly' });
            }

            pc.ontrack = (event) => {
                let container = document.getElementById(`wrapper-${userId}`);
                if (!container) {
                    container = document.createElement('div');
                    container.id = `wrapper-${userId}`;
                    container.className = 'video-container group bg-slate-800 border border-slate-700';
                    
                    const vid = document.createElement('video');
                    vid.id = `vid-${userId}`;
                    vid.autoplay = true;
                    vid.playsInline = true; 
                    attachSinkId(vid, currentAudioOutputId);
                    vid.srcObject = new MediaStream();
                    
                    const avatarLayer = document.createElement('div');
                    avatarLayer.className = 'avatar-layer';
                    
                    setAvatar(avatarLayer, avatarUrl);

                    const label = document.createElement('div');
                    label.className = 'absolute bottom-3 left-3 bg-black/50 px-3 py-1 rounded-full text-sm text-white backdrop-blur-md z-30';
                    label.innerText = displayName;

                    const volControls = document.createElement('div');
                    volControls.id = `vol-controls-${userId}`;
                    volControls.className = 'volume-controls z-30';
                    
                    const fsBtn = document.createElement('button');
                    fsBtn.className = 'absolute top-3 right-3 p-2 rounded-xl bg-black/40 hover:bg-blue-600 text-white backdrop-blur-md transition-all opacity-0 group-hover:opacity-100 scale-90 hover:scale-100 z-30';
                    fsBtn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2-2h3"/></svg>';
                    fsBtn.onclick = () => toggleFullscreen(userId);
                    fsBtn.title = "Toggle Fullscreen";
                    
                    container.addEventListener('fullscreenchange', () => {
                        if (document.fullscreenElement === container) {
                            fsBtn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3v3a2 2 0 0 1-2 2H3m18 0h-3a2 2 0 0 1-2-2V3m0 18v-3a2 2 0 0 1 2-2h3"/></svg>';
                            fsBtn.classList.add('bg-blue-600');
                        } else {
                            fsBtn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2-2h3"/></svg>';
                            fsBtn.classList.remove('bg-blue-600');
                        }
                    });

                    container.appendChild(vid); 
                    container.appendChild(avatarLayer);
                    container.appendChild(label);
                    container.appendChild(volControls);
                    container.appendChild(fsBtn);
                    remoteGrid.appendChild(container);
                    checkEmpty();
                }

                const vid = document.getElementById(`vid-${userId}`);
                const volControls = document.getElementById(`vol-controls-${userId}`);
                const mainStream = vid.srcObject;

                if (event.track.kind === 'video') {
                     mainStream.getVideoTracks().forEach(t => mainStream.removeTrack(t));
                     mainStream.addTrack(event.track);
                     vid.play().catch(e => console.error("Remote play err", e));
                     
                     event.track.onmute = () => { checkActive(userId); };
                     event.track.onunmute = () => { checkActive(userId); };
                     event.track.onended = () => { checkActive(userId); };
                }
                
                if (event.track.kind === 'audio') {
                    if (mainStream.getAudioTracks().length === 0) {
                        mainStream.addTrack(event.track);
                        setupAudioMonitor(mainStream, `wrapper-${userId}`);
                        
                        const row = document.createElement('div');
                        row.className = 'vol-row';
                        row.id = `vol-row-main-${userId}`;
                        row.innerHTML = `
                            <button class="text-white hover:text-blue-400" onclick="toggleMute('${userId}', 'main')" id="mute-main-${userId}">
                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>
                            </button>
                            <input type="range" min="0" max="1" step="0.05" value="1" oninput="setVolume('${userId}', 'main', this.value)">
                        `;
                        volControls.insertBefore(row, volControls.firstChild);
                        
                        event.track.onended = () => {
                            row.remove();
                        };
                    } else {
                        const screenStream = new MediaStream([event.track]);
                        const audEl = new Audio();
                        audEl.srcObject = screenStream;
                        audEl.id = `aud-screen-${userId}`;
                        audEl.autoplay = true;
                        attachSinkId(audEl, currentAudioOutputId);
                        container.appendChild(audEl);
                        
                        const row = document.createElement('div');
                        row.className = 'vol-row';
                        row.id = `vol-row-screen-${userId}`;
                        row.innerHTML = `
                             <button class="text-white hover:text-purple-400" onclick="toggleMute('${userId}', 'screen')" id="mute-screen-${userId}" title="Screen Audio">
                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="2" width="16" height="14" rx="2" ry="2"></rect><line x1="12" y1="22" x2="12" y2="16"></line><path d="M5 12h14"></path><path d="M12 12v4"></path></svg>
                            </button>
                            <input type="range" min="0" max="1" step="0.05" value="1" oninput="setVolume('${userId}', 'screen', this.value)">
                        `;
                        volControls.appendChild(row);
                        
                        event.track.onended = () => {
                            audEl.remove();
                            row.remove();
                        };
                    }
                }
                
                const checkActive = (uid) => {
                     const v = document.getElementById(`vid-${uid}`);
                     if (!v || !v.srcObject) return;
                     
                     const isCamOff = peerCamStatus[uid] === false;
                     const isScreenOn = peerScreenStatus[uid] === true;

                     if (isScreenOn) {
                         v.classList.add('active');
                         v.style.objectFit = 'contain';
                         return;
                     }

                     if (isCamOff) {
                         v.classList.remove('active');
                         return;
                     }

                     const vTracks = v.srcObject.getVideoTracks();
                     let hasActiveVideo = false;
                     if (vTracks.length > 0) {
                         const t = vTracks[0];
                         if (t.enabled && !t.muted && t.readyState === 'live') {
                             hasActiveVideo = true;
                         }
                     }

                     if (hasActiveVideo) {
                         v.classList.add('active');
                         v.style.objectFit = 'contain';
                     } else {
                         v.classList.remove('active');
                     }
                };
                
                if (event.track.kind === 'video') {
                     vid.onloadedmetadata = () => checkActive(userId);
                     vid.onresize = () => checkActive(userId);
                }
                
                if (!container.dataset.interval) {
                    const intId = setInterval(() => checkActive(userId), 1000);
                    container.dataset.interval = intId;
                }
            };

            pc.onicecandidate = (event) => {
                if (event.candidate) {
                    sendSignal(userId, { type: 'candidate', candidate: event.candidate });
                }
            };

            if (initiator) {
                negotiate(userId, pc);
            }
        }

        async function handleSignal(userId, data) {
            if (!peers[userId]) initPeer(userId, false, "Unknown", null); 
            const pc = peers[userId];

            try {
                if (data.type === 'offer') {
                    await pc.setRemoteDescription(new RTCSessionDescription(data.sdp));
                    const answer = await pc.createAnswer();
                    answer.sdp = forceStereoAudio(answer.sdp);
                    await pc.setLocalDescription(answer);
                    sendSignal(userId, { type: 'answer', sdp: answer });
                } else if (data.type === 'answer') {
                    await pc.setRemoteDescription(new RTCSessionDescription(data.sdp));
                } else if (data.type === 'candidate') {
                    await pc.addIceCandidate(new RTCIceCandidate(data.candidate));
                }
            } catch (e) {
                console.error("Signaling error", e);
            }
        }

        function removePeer(userId) {
            if (peers[userId]) {
                peers[userId].close();
                delete peers[userId];
            }
            const el = document.getElementById(`wrapper-${userId}`);
            if (el) el.remove();
            checkEmpty();
        }

        function sendSignal(toId, data) {
            ws.send(JSON.stringify({ type: 'signal', target: toId, data: data }));
        }


        window.toggleFullscreen = function(userId) {
            const el = document.getElementById(`wrapper-${userId}`);
            if (!el) return;
            
            if (!document.fullscreenElement) {
                el.requestFullscreen().catch(err => {
                    console.error(`Error attempting to enable fullscreen: ${err.message}`);
                });
            } else {
                document.exitFullscreen();
            }
        };

        window.toggleMute = function(userId, type) {
            let el;
            let btn;
            
            if (type === 'screen') {
                el = document.getElementById(`aud-screen-${userId}`);
                btn = document.getElementById(`mute-screen-${userId}`);
            } else {
                el = document.getElementById(`vid-${userId}`);
                btn = document.getElementById(`mute-main-${userId}`);
            }

            if (el) {
                el.muted = !el.muted;
                const isMuted = el.muted;
                
                if (type === 'screen') {
                     if (isMuted) {
                        btn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="2" width="16" height="14" rx="2" ry="2"></rect><line x1="23" y1="9" x2="17" y2="15"></line><line x1="17" y1="9" x2="23" y2="15"></line></svg>`;
                        btn.classList.add('text-red-500');
                    } else {
                        btn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="2" width="16" height="14" rx="2" ry="2"></rect><line x1="12" y1="22" x2="12" y2="16"></line><path d="M5 12h14"></path><path d="M12 12v4"></path></svg>`;
                        btn.classList.remove('text-red-500');
                    }
                } else {
                    if (isMuted) {
                        btn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><line x1="23" y1="9" x2="17" y2="15"></line><line x1="17" y1="9" x2="23" y2="15"></line></svg>`;
                        btn.classList.add('text-red-500');
                    } else {
                        btn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`;
                        btn.classList.remove('text-red-500');
                    }
                }
            }
        }

        window.setVolume = function(userId, type, val) {
             let el;
            if (type === 'screen') {
                el = document.getElementById(`aud-screen-${userId}`);
            } else {
                el = document.getElementById(`vid-${userId}`);
            }
            if (el) {
                el.volume = val;
            }
        }

        function toggleMic() {
            if (!localStream) return;
            const tracks = localStream.getAudioTracks();
            if (tracks.length > 0) {
                const track = tracks[0];
                track.enabled = !track.enabled;
                const btn = document.getElementById('btnMic');
                if (!track.enabled) {
                    btn.classList.add('active-red');
                    btn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="1" y1="1" x2="23" y2="23"></line><path d="M9 9v3a3 3 0 0 0 5.12 2.12M15 9.34V4a3 3 0 0 0-5.94-.6"></path><path d="M17 16.95A7 7 0 0 1 5 12v-2m14 0v2a7 7 0 0 1-.11 1.23"></path><line x1="12" x2="12" y1="19" y2="22"></line></svg>`;
                } else {
                    btn.classList.remove('active-red');
                    btn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" x2="12" y1="19" y2="22"/></svg>`;
                }
                updateLocalLabel();
            }
        }

        async function toggleCam() {
            const btn = document.getElementById('btnCam');
            if (!localStream) return;
            
            let tracks = localStream.getVideoTracks();
            let justAdded = false;
            
            if (tracks.length === 0) {
                try {
                    const newStream = await navigator.mediaDevices.getUserMedia({ video: true });
                    const newTrack = newStream.getVideoTracks()[0];
                    localStream.addTrack(newTrack);
                    tracks = localStream.getVideoTracks();
                    justAdded = true;

                    if (!screenStream) {
                        for (const userId in peers) {
                            const pc = peers[userId];
                            pc.addTrack(newTrack, localStream);
                            negotiate(userId, pc);
                        }
                    }
                } catch (e) {
                    console.error("Could not add camera", e);
                    alert("Could not access camera. Please check permissions.");
                    return;
                }
            }

            if (tracks.length > 0) {
                const track = tracks[0];
                if (!justAdded) {
                    track.enabled = !track.enabled;
                }
                
                if (!track.enabled) {
                    btn.classList.add('active-red');
                    btn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="1" y1="1" x2="23" y2="23"></line><path d="M21 21l-3.5-3.5m-2-2l-2-2m-2-2l-2-2m-2-2l-3.5-3.5"></path><path d="M15 7h5a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-5"></path><path d="M4 8v8a2 2 0 0 0 2 2h4.5"></path></svg>`;
                } else {
                    btn.classList.remove('active-red');
                    btn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z"/><circle cx="12" cy="13" r="3"/></svg>`;
                }
                updateLocalAvatar();
                
                if (ws && ws.readyState === WebSocket.OPEN) {
                    ws.send(JSON.stringify({
                        type: 'cam-toggle',
                        data: { enabled: track.enabled }
                    }));
                }
            }
        }

        async function toggleScreen() {
            const btn = document.getElementById('btnShare');
            
            if (screenStream) {
                let videoTrack = localStream ? localStream.getVideoTracks()[0] : null;
                const screenAudioTrack = screenStream.getAudioTracks()[0];

                screenStream.getTracks().forEach(t => t.stop());
                screenStream = null;
                btn.classList.remove('active-green');
                
                if (localStream) {
                    localVideo.srcObject = localStream;
                } else {
                    localVideo.srcObject = null;
                }
                
                if (ws && ws.readyState === WebSocket.OPEN) {
                    ws.send(JSON.stringify({
                        type: 'screen-toggle',
                        data: { enabled: false, hasAudio: false }
                    }));
                }

                for (const userId in peers) {
                    const pc = peers[userId];
                    const senders = pc.getSenders();
                    let shouldNegotiate = false;
                    
                    const vidSender = senders.find(s => s.track && s.track.kind === 'video');
                    if (vidSender) {
                        if (videoTrack) {
                            vidSender.replaceTrack(videoTrack);
                        } else {
                            pc.removeTrack(vidSender);
                            shouldNegotiate = true;
                        }
                    }

                    if (screenAudioTrack) {
                        const audSender = senders.find(s => s.track && s.track.id === screenAudioTrack.id);
                        if (audSender) {
                            pc.removeTrack(audSender);
                            shouldNegotiate = true;
                        }
                    }
                    
                    if (shouldNegotiate) {
                        negotiate(userId, pc);
                    }
                }

                updateLocalAvatar();

            } else {
                try {
                    screenStream = await navigator.mediaDevices.getDisplayMedia({ 
                        video: { cursor: true },
                        systemAudio: "include",
                        audio: {
                            echoCancellation: true,
                            noiseSuppression: false,
                            autoGainControl: false,
                            channelCount: 2,
                            sampleRate: 48000,
                            sampleSize: 16
                        } 
                    });
                    const screenTrack = screenStream.getVideoTracks()[0];
                    const screenAudioTrack = screenStream.getAudioTracks()[0];
                    
                    if (screenAudioTrack) {
                        screenAudioTrack.contentHint = "music";
                    }

                    localVideo.srcObject = screenStream;
                    
                    updateLocalAvatar();

                    if (ws && ws.readyState === WebSocket.OPEN) {
                        ws.send(JSON.stringify({
                            type: 'screen-toggle',
                            data: { enabled: true, hasAudio: !!screenAudioTrack }
                        }));
                    }

                    for (const userId in peers) {
                        const pc = peers[userId];
                        const senders = pc.getSenders();
                        const vidSender = senders.find(s => s.track && s.track.kind === 'video');
                        let shouldNegotiate = false;
                        
                        if (vidSender) {
                            vidSender.replaceTrack(screenTrack);
                        } else {
                            if (localStream) {
                                pc.addTrack(screenTrack, localStream);
                            } else {
                                pc.addTrack(screenTrack, screenStream);
                            }
                            shouldNegotiate = true;
                        }

                        if (screenAudioTrack) {
                            let sender = pc.addTrack(screenAudioTrack, screenStream);

                            const params = sender.getParameters();
                            if (!params.encodings) params.encodings = [{}];
                            params.encodings[0].maxBitrate = 512000;
                            sender.setParameters(params).catch(e => console.warn(e));

                            shouldNegotiate = true;
                        }
                        
                        if (shouldNegotiate) {
                            negotiate(userId, pc);
                        }
                    }

                    screenTrack.onended = () => { toggleScreen(); };
                    btn.classList.add('active-green');
                } catch (e) {
                    console.error("Screen share failed", e);
                }
            }
        }

        function updateLocalLabel() {
            const label = document.getElementById('localLabel');
            if (!label) return;
            if (!localStream) {
                label.innerText = "You (Offline)";
                return;
            }
            const audioTrack = localStream.getAudioTracks()[0];
            if (audioTrack && audioTrack.enabled) {
                label.innerText = `You (${userNickname})`;
            } else {
                label.innerText = `You (Muted)`;
            }
        }

        function copyLink() {
            navigator.clipboard.writeText(window.location.href);
            
            const btn = document.getElementById('btnCopy');
            const icon = document.getElementById('iconCopy');
            
            const originalHTML = btn.innerHTML;
            const originalClass = btn.className;
            
            btn.innerHTML = `<span class="text-xs md:text-sm font-medium text-white">Copied!</span><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`;
            btn.classList.add('bg-green-600', 'hover:bg-green-700');
            btn.classList.remove('hover:bg-slate-700/50');

            setTimeout(() => {
                btn.innerHTML = originalHTML;
                btn.className = originalClass;
            }, 2000);
        }

        const settingsOverlay = document.getElementById('settingsOverlay');
        const settingsNicknameInput = document.getElementById('settingsNicknameInput');
        const settingsAvatarInput = document.getElementById('settingsAvatarInput');
        const settingsAvatarPreview = document.getElementById('settingsAvatarPreview');
        const settingsAvatarPlaceholder = document.getElementById('settingsAvatarPlaceholder');
        let newAvatarCandidate = null;

        function openSettings() {
            settingsNicknameInput.value = userNickname;
            newAvatarCandidate = userAvatar;
            
            if (userAvatar) {
                settingsAvatarPreview.src = userAvatar;
                settingsAvatarPreview.classList.remove('hidden');
                settingsAvatarPlaceholder.classList.add('hidden');
            } else {
                settingsAvatarPreview.classList.add('hidden');
                settingsAvatarPlaceholder.classList.remove('hidden');
            }
            
            populateSettingsDeviceList();
            settingsOverlay.classList.remove('hidden');
        }

        function closeSettings() {
            settingsOverlay.classList.add('hidden');
        }

        function handleSettingsAvatarUpload(input) {
            const file = input.files[0];
            if (!file) return;

            const reader = new FileReader();
            reader.onload = function(e) {
                const img = new Image();
                img.onload = function() {
                    const canvas = document.createElement('canvas');
                    const ctx = canvas.getContext('2d');
                    const MAX_SIZE = 128;
                    let width = img.width;
                    let height = img.height;
                    
                    if (width > height) {
                        if (width > MAX_SIZE) {
                            height *= MAX_SIZE / width;
                            width = MAX_SIZE;
                        }
                    } else {
                        if (height > MAX_SIZE) {
                            width *= MAX_SIZE / height;
                            height = MAX_SIZE;
                        }
                    }
                    
                    canvas.width = width;
                    canvas.height = height;
                    ctx.drawImage(img, 0, 0, width, height);
                    
                    newAvatarCandidate = canvas.toDataURL('image/jpeg', 0.8);
                    settingsAvatarPreview.src = newAvatarCandidate;
                    settingsAvatarPreview.classList.remove('hidden');
                    settingsAvatarPlaceholder.classList.add('hidden');
                };
                img.src = e.target.result;
            };
            reader.readAsDataURL(file);
        }

        async function saveSettings() {
            const newAudio = document.getElementById('settingsAudioSource').value;
            const newAudioOutput = document.getElementById('settingsAudioOutputSource').value;
            const newVideo = document.getElementById('settingsVideoSource').value;
            
            const currentAudioTrack = localStream ? localStream.getAudioTracks()[0] : null;
            const currentVideoTrack = localStream ? localStream.getVideoTracks()[0] : null;
            
            const currentAudioId = currentAudioTrack ? currentAudioTrack.getSettings().deviceId : "";
            const currentVideoId = currentVideoTrack ? currentVideoTrack.getSettings().deviceId : "";

            if (newAudio !== currentAudioId || newVideo !== currentVideoId) {
                await switchMediaStream(newAudio, newVideo);
            }

            if (newAudioOutput !== currentAudioOutputId) {
                await changeAudioOutput(newAudioOutput);
            }

            userNickname = settingsNicknameInput.value.trim() || "Guest";
            userAvatar = newAvatarCandidate;
            savePreferences();
            
            updateLocalLabel();
            updateLocalAvatar();
            
            if (ws && ws.readyState === WebSocket.OPEN) {
                 ws.send(JSON.stringify({
                    type: "update-user", 
                    data: {
                        nickname: userNickname,
                        avatar: userAvatar 
                    } 
                }));
            }
            
            closeSettings();
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
                     img.classList.remove('hidden');
                     
                     centerImg.src = userAvatar;
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
            
            let isDragging = false;
            let dragOffset = { x: 0, y: 0 };
            
            function startDrag(clientX, clientY) {
                isDragging = true;
                pip.style.cursor = 'grabbing';
                
                const rect = pip.getBoundingClientRect();
                
                pip.style.bottom = 'auto';
                pip.style.right = 'auto';
                pip.style.left = rect.left + 'px';
                pip.style.top = rect.top + 'px';
                
                dragOffset.x = clientX - rect.left;
                dragOffset.y = clientY - rect.top;
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
                if (!isDragging) return;
                
                let newX = clientX - dragOffset.x;
                let newY = clientY - dragOffset.y;
                
                const pipRect = pip.getBoundingClientRect();
                const taskbarRect = taskbar.getBoundingClientRect();
                const margin = 16;
                
                const minX = margin;
                const maxX = window.innerWidth - pipRect.width - margin;
                const minY = margin;
                const maxY = window.innerHeight - taskbarRect.height - pipRect.height - margin;
                
                newX = Math.max(minX, Math.min(newX, maxX));
                newY = Math.max(minY, Math.min(newY, maxY));
                
                if (connectionDot && connectionDot.parentElement) {
                    const statusRect = connectionDot.parentElement.getBoundingClientRect();
                    const dangerRight = statusRect.right + margin;
                    const dangerBottom = statusRect.bottom + margin;
                    
                    if (newX < dangerRight && newY < dangerBottom) {
                        const distToRight = dangerRight - newX;
                        const distToBottom = dangerBottom - newY;
                        if (distToRight < distToBottom) newX = dangerRight;
                        else newY = dangerBottom;
                    }
                }
                
                if (btnCopy) {
                    const copyRect = btnCopy.getBoundingClientRect();
                    const dangerLeft = copyRect.left - margin - pipRect.width;
                    const dangerBottom = copyRect.bottom + margin;
                    
                    if (newX > dangerLeft && newY < dangerBottom) {
                        const distToLeft = newX - dangerLeft;
                        const distToBottom = dangerBottom - newY;
                        if (distToLeft < distToBottom) newX = dangerLeft;
                        else newY = dangerBottom;
                    }
                }
                
                pip.style.left = newX + 'px';
                pip.style.top = newY + 'px';
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
                document.removeEventListener('mousemove', onMouseMove);
                document.removeEventListener('mouseup', onMouseUp);
            }
            
            function onTouchEnd() {
                isDragging = false;
                pip.style.cursor = 'grab';
                document.removeEventListener('touchmove', onTouchMove);
                document.removeEventListener('touchend', onTouchEnd);
                document.removeEventListener('touchcancel', onTouchEnd);
            }
            
            pip.addEventListener('mousedown', onMouseDown);
            pip.addEventListener('touchstart', onTouchStart, { passive: false });

            window.addEventListener('resize', () => {
                if (!pip.style.left) return;

                const pipRect = pip.getBoundingClientRect();
                const taskbarRect = taskbar.getBoundingClientRect();
                const margin = 16;

                const minX = margin;
                const maxX = window.innerWidth - pipRect.width - margin;
                const minY = margin;
                const maxY = window.innerHeight - taskbarRect.height - pipRect.height - margin;
                
                let currentLeft = parseFloat(pip.style.left);
                let currentTop = parseFloat(pip.style.top);
                
                if (isNaN(currentLeft) || isNaN(currentTop)) return;

                let newX = Math.max(minX, Math.min(currentLeft, maxX));
                let newY = Math.max(minY, Math.min(currentTop, maxY));
                
                pip.style.left = newX + 'px';
                pip.style.top = newY + 'px';
            });
        })();
    </script>
</body>
</html>
"###;
    html.replace("{{TURN_USER}}", turn_user).replace("{{TURN_PASS}}", turn_pass)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignalMessage {
    #[serde(rename = "type")]
    msg_type: String,     
    target: Option<String>,
    data: Option<serde_json::Value>, 
    #[serde(rename = "userId")]
    user_id: Option<String>, 
}

type UserTx = tokio::sync::mpsc::UnboundedSender<Result<Message, axum::Error>>;
type RoomMap = Arc<Mutex<HashMap<String, HashMap<String, UserTx>>>>;

#[derive(Clone)]
struct AppState {
    rooms: RoomMap,
    turn_user: String,
    turn_pass: String,
}

#[tokio::main]
async fn main() {
    let rooms: RoomMap = Arc::new(Mutex::new(HashMap::new()));
    
    let turn_user = Uuid::new_v4().to_string();
    let turn_pass = Uuid::new_v4().to_string();
    let realm = "rustrooms";

    let state = AppState {
        rooms,
        turn_user: turn_user.clone(),
        turn_pass: turn_pass.clone(),
    };

    let app = Router::new()
        .route("/", get(serve_room))
        .route("/new", get(redirect_to_new_room))
        .route("/room/:id", get(serve_room))
        .route("/ws/:id", get(ws_handler))
        .with_state(state);

    let t_user = turn_user.clone();
    let t_pass = turn_pass.clone();
    
    tokio::spawn(async move {
        if let Err(e) = turn_server::start(3478, t_user, t_pass, realm.to_string()).await {
            eprintln!("failed to start TURN server: {}", e);
        }
    });

    let port = 3000;
    let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ERROR: Failed to bind to port {}: {}", port, e);
            eprintln!("Is the server already running? Try killing the process using this port.");
            std::process::exit(1);
        }
    };
    println!("SERVER RUNNING ON PORT {}", port);
    axum::serve(listener, app).await.unwrap();
}

async fn redirect_to_new_room() -> Redirect {
    let new_id = Uuid::new_v4().to_string();
    Redirect::to(&format!("/room/{}", new_id))
}

async fn serve_room(State(state): State<AppState>) -> Html<String> {
    Html(get_html_page(&state.turn_user, &state.turn_pass))
}

async fn ws_handler(
    Path(room_id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, room_id, state.rooms))
}

async fn handle_socket(socket: WebSocket, room_id: String, rooms: RoomMap) {
    let (mut user_ws_tx, mut user_ws_rx) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    
    let mut user_id = String::new(); 
    let mut is_joined = false;

    tokio::spawn(async move {
        while let Some(result) = rx.recv().await {
            if let Ok(msg) = result {
                if user_ws_tx.send(msg).await.is_err() {
                    break;
                }
            }
        }
    });

    while let Some(result) = user_ws_rx.next().await {
        if let Ok(msg) = result {
            if let Message::Text(text) = msg {
                if let Ok(parsed) = serde_json::from_str::<SignalMessage>(&text) {
                    if !is_joined {
                        if parsed.msg_type == "join" {
                             user_id = parsed.user_id.unwrap_or_else(|| Uuid::new_v4().to_string());
                             
                             {
                                let mut rooms_lock = rooms.lock().await;
                                let room = rooms_lock.entry(room_id.clone()).or_insert_with(HashMap::new);
                                
                                if room.contains_key(&user_id) {
                                    break; 
                                }
                                
                                room.insert(user_id.clone(), tx.clone());
                             }
                             is_joined = true;
                             
                             let notify_data = parsed.data.clone();
                             let notify_msg = serde_json::to_string(&SignalMessage {
                                msg_type: "user-joined".into(),
                                user_id: Some(user_id.clone()),
                                target: None,
                                data: notify_data,
                            }).unwrap();

                            let rooms_lock = rooms.lock().await;
                            if let Some(room) = rooms_lock.get(&room_id) {
                                for (uid, tx) in room.iter() {
                                    if *uid != user_id {
                                        let _ = tx.send(Ok(Message::Text(notify_msg.clone().into())));
                                    }
                                }
                            }
                        }
                    } else {
                        let rooms_lock = rooms.lock().await;
                        if let Some(room) = rooms_lock.get(&room_id) {
                            if parsed.msg_type == "update-user" {
                                let notify_data = parsed.data.clone();
                                let notify_msg = serde_json::to_string(&SignalMessage {
                                    msg_type: "user-update".into(),
                                    user_id: Some(user_id.clone()),
                                    target: None,
                                    data: notify_data,
                                }).unwrap();

                                for (uid, tx) in room.iter() {
                                    if *uid != user_id {
                                        let _ = tx.send(Ok(Message::Text(notify_msg.clone().into())));
                                    }
                                }
                            } else if parsed.msg_type == "cam-toggle" {
                                let notify_data = parsed.data.clone();
                                let notify_msg = serde_json::to_string(&SignalMessage {
                                    msg_type: "cam-toggle".into(),
                                    user_id: Some(user_id.clone()),
                                    target: None,
                                    data: notify_data,
                                }).unwrap();

                                for (uid, tx) in room.iter() {
                                    if *uid != user_id {
                                        let _ = tx.send(Ok(Message::Text(notify_msg.clone().into())));
                                    }
                                }
                            } else if parsed.msg_type == "screen-toggle" {
                                let notify_data = parsed.data.clone();
                                let notify_msg = serde_json::to_string(&SignalMessage {
                                    msg_type: "screen-toggle".into(),
                                    user_id: Some(user_id.clone()),
                                    target: None,
                                    data: notify_data,
                                }).unwrap();

                                for (uid, tx) in room.iter() {
                                    if *uid != user_id {
                                        let _ = tx.send(Ok(Message::Text(notify_msg.clone().into())));
                                    }
                                }
                            } else if let Some(ref target_id) = parsed.target {
                                if let Some(target_tx) = room.get(target_id) {
                                    let mut forwarded_msg = parsed.clone();
                                    forwarded_msg.user_id = Some(user_id.clone());
                                    let forwarded_text = serde_json::to_string(&forwarded_msg).unwrap();
                                    let _ = target_tx.send(Ok(Message::Text(forwarded_text.into())));
                                }
                            }
                        }
                    }
                }
            } else if let Message::Close(_) = msg {
                break;
            }
        } else {
            break;
        }
    }

    {
        let mut rooms_lock = rooms.lock().await;
        if let Some(room) = rooms_lock.get_mut(&room_id) {
            if is_joined {
                room.remove(&user_id);
                if room.is_empty() {
                    rooms_lock.remove(&room_id);
                } else {
                    let notify_msg = serde_json::to_string(&SignalMessage {
                        msg_type: "user-left".into(),
                        user_id: Some(user_id.clone()),
                        target: None,
                        data: None,
                    }).unwrap();

                    for (_, tx) in room.iter() {
                        let _ = tx.send(Ok(Message::Text(notify_msg.clone().into())));
                    }
                }
            }
        }
    }
}