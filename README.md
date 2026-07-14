# Sentinel Streaming

**Sentinel Streaming** is a high-performance, camera-first streaming platform written in Rust.

It provides a unified way to ingest, stream, record, and expose video from multiple camera sources through a simple API. Designed for AI, surveillance, robotics, and edge computing, Sentinel Streaming serves as the media foundation for the Sentinel ecosystem while remaining useful as a standalone open-source project.

## Goals

- Camera-first architecture
- High-performance Rust implementation
- AI-ready frame access
- Cross-platform
- Edge-friendly
- Simple REST API
- Extensible through SDKs

---

# Features

## Camera Support

- [ ] Built-in camera
- [ ] USB camera
- [ ] RTSP cameras
- [ ] ONVIF discovery
- [ ] IP/Wi-Fi cameras
- [ ] Video file playback
- [ ] Multi-camera support

## Streaming

- [ ] Live video streaming
- [ ] Browser viewer
- [ ] MJPEG streaming
- [ ] HLS streaming
- [ ] WebRTC
- [ ] Adaptive streaming

## Recording

- [ ] Continuous recording
- [ ] Event recording
- [ ] Snapshot capture
- [ ] Video retention policies
- [ ] Video export

## AI Integration

- [ ] Frame access API
- [ ] Frame subscription
- [ ] Motion event publishing
- [ ] Metadata streaming
- [ ] Zero-copy frame pipeline

## API

- [ ] REST API
- [ ] WebSocket events
- [ ] OpenAPI documentation
- [ ] Authentication
- [ ] Health endpoints
- [ ] Metrics

## SDKs

- [ ] Rust
- [ ] Java
- [ ] Python
- [ ] Node.js

---

# Development Priority

## Phase 1 — MVP

- [ ] Built-in camera support
- [ ] Live browser streaming
- [ ] Start/Stop camera
- [ ] Snapshot endpoint
- [ ] Stream status API
- [ ] Basic health monitoring

## Phase 2 — Camera Platform

- [ ] RTSP support
- [ ] USB camera support
- [ ] Multiple cameras
- [ ] Recording
- [ ] Camera management

## Phase 3 — AI Platform

- [ ] Frame subscription API
- [ ] Metadata pipeline
- [ ] Motion events
- [ ] AI integration interfaces

## Phase 4 — Enterprise

- [ ] ONVIF discovery
- [ ] Authentication
- [ ] Authorization
- [ ] WebRTC
- [ ] Horizontal scaling
- [ ] High availability
- [ ] Distributed streaming

---

# Out of Scope

Sentinel Streaming intentionally does **not** implement:

- Intrusion detection
- Object detection
- Face recognition
- Security rules
- Alerting
- Home automation
- Traffic analytics

These capabilities belong to products built on top of Sentinel Streaming.

---

# Architecture

```
Camera Sources
        │
        ▼
Camera Adapters
        │
        ▼
Streaming Pipeline
        │
 ┌──────┼────────┐
 ▼      ▼        ▼
Live  Recording  Snapshots
 │
 ▼
REST / WebSocket APIs
 │
 ▼
Applications & AI
```

---

# License

Apache 2.0
