#!/usr/bin/env python3
"""
deepface_cli.py

Refactored DeepFace CLI + WebSocket server for use as a child process
(e.g. Tauri sidecar). Works both as single-run CLI and as a long-running
WebSocket worker.

Modes:
    - CLI subcommands (default): analyze, verify, detect, find.
    - Serve mode:
      * `serve` command starts a WebSocket server (default host=127.0.0.1)
      * Use `--port` to choose the port

Protocol (WebSocket):
    * Each message must be a JSON object. Example:
        {"requestId":1, "cmd":"analyze", "frame":"path1", "actions":"emotion", "detector":"opencv"}

    * Responses are JSON messages with structure:
        { "requestId": <id>, "status": "ok|error", "command": "<cmd>", "data": <payload> }

Design:
    - Always processes one frame per request (no bulk).
    - stdout is NOT used by WebSocket mode. For CLI, stdout contains the final JSON.
    - stderr is reserved for logs / debug statements.
    - safe_call helper retries calls if DeepFace API has different kwargs.
"""

import logging
import os
import sys
import json
import argparse
import asyncio
import traceback
from typing import Any, Dict

# third-party
try:
    import websockets
except Exception:
    print("Missing dependency: websockets (pip install websockets).", file=sys.stderr)
    raise

try:
    from deepface import DeepFace
except Exception:
    print("Missing dependency: deepface (pip install deepface).", file=sys.stderr)
    raise

import numpy as np

# ----------------------------
# PyInstaller support
# ----------------------------
if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
    meipass_weights = os.path.join(sys._MEIPASS, "deepface_weights")
    os.environ["DEEPFACE_HOME"] = meipass_weights
else:
    os.environ.setdefault("DEEPFACE_HOME", os.path.abspath("deepface_weights"))

# ----------------------------
# Utilities
# ----------------------------
def eprint(*args, **kwargs):
    """Print to stderr for logs."""
    print(*args, file=sys.stderr, **kwargs)

def make_serializable(obj: Any) -> Any:
    """Convert numpy / tensorflow / sets / bytes into JSON-safe Python types."""
    if isinstance(obj, np.generic):
        return obj.item()
    if isinstance(obj, np.ndarray):
        return obj.tolist()
    if isinstance(obj, (bytes, bytearray)):
        try:
            return obj.decode("utf-8")
        except Exception:
            return list(obj)
    if isinstance(obj, dict):
        return {str(k): make_serializable(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple, set)):
        return [make_serializable(v) for v in obj]
    try:
        json.dumps(obj)
        return obj
    except Exception:
        return str(obj)

def safe_call(func, kwargs: Dict[str, Any]):
    """Call DeepFace function with kwargs, dropping unsupported args if necessary."""
    try:
        return func(**kwargs)
    except TypeError as e:
        msg = str(e)
        if "unexpected keyword" in msg:
            new_kwargs = dict(kwargs)
            for k in ["model", "model_name", "model_name_or_model", "models"]:
                if k in new_kwargs:
                    new_kwargs.pop(k)
                    try:
                        return func(**new_kwargs)
                    except TypeError:
                        pass
        raise

# ----------------------------
# Command handlers (single-frame only)
# ----------------------------
def cmd_analyze(args_or_req) -> Any:
    """
    Analyze a single frame.

    Args:
        frame / --frame: path to image
        actions: "emotion,age,gender"
        detector: backend string
        model: optional model name
        enforce_detection: bool
    """
    if isinstance(args_or_req, argparse.Namespace):
        frame = getattr(args_or_req, "frame", None)
        if not frame and getattr(args_or_req, "frames", None):
            frame = args_or_req.frames[0]
        actions = getattr(args_or_req, "actions", None)
        detector = getattr(args_or_req, "detector", None)
        enforce_detection = getattr(args_or_req, "enforce_detection", False)
        model = getattr(args_or_req, "model", None)
    elif isinstance(args_or_req, dict):
        frame = args_or_req.get("frame") or (args_or_req.get("frames") or [None])[0]
        actions = args_or_req.get("actions")
        detector = args_or_req.get("detector")
        enforce_detection = args_or_req.get("enforce_detection", False)
        model = args_or_req.get("model")
    else:
        raise ValueError("Unsupported input type for cmd_analyze")

    if not frame:
        raise ValueError("No frame provided")

    kwargs = {"img_path": frame, "enforce_detection": enforce_detection}
    if actions:
        kwargs["actions"] = [a.strip() for a in actions.split(",") if a.strip()]
    if detector:
        kwargs["detector_backend"] = detector
    if model:
        kwargs["model_name"] = model
        kwargs["model"] = model

    return {"frame": frame, "result": safe_call(DeepFace.analyze, kwargs)}

def cmd_verify(args_or_req) -> Any:
    """Verify: compare two images."""
    if isinstance(args_or_req, argparse.Namespace):
        img1 = getattr(args_or_req, "img1", None)
        img2 = getattr(args_or_req, "img2", None)
        detector = getattr(args_or_req, "detector", None)
        enforce_detection = getattr(args_or_req, "enforce_detection", False)
        model = getattr(args_or_req, "model", None)
    elif isinstance(args_or_req, dict):
        img1 = args_or_req.get("img1")
        img2 = args_or_req.get("img2")
        detector = args_or_req.get("detector")
        enforce_detection = args_or_req.get("enforce_detection", False)
        model = args_or_req.get("model")
    else:
        raise ValueError("Unsupported input type for cmd_verify")

    if not img1 or not img2:
        raise ValueError("verify requires img1 and img2")

    kwargs = {"img1_path": img1, "img2_path": img2, "enforce_detection": enforce_detection}
    if detector:
        kwargs["detector_backend"] = detector
    if model:
        kwargs["model_name"] = model
        kwargs["model"] = model

    return safe_call(DeepFace.verify, kwargs)

def cmd_detect(args_or_req) -> Any:
    """Detect faces from a single frame."""
    if isinstance(args_or_req, argparse.Namespace):
        frame = getattr(args_or_req, "frame", None) or (getattr(args_or_req, "frames", None) or [None])[0]
        detector = getattr(args_or_req, "detector", None)
        enforce_detection = getattr(args_or_req, "enforce_detection", False)
    elif isinstance(args_or_req, dict):
        frame = args_or_req.get("frame") or (args_or_req.get("frames") or [None])[0]
        detector = args_or_req.get("detector")
        enforce_detection = args_or_req.get("enforce_detection", False)
    else:
        raise ValueError("Unsupported input type for cmd_detect")

    if not frame:
        raise ValueError("No frame provided")

    return {"frame": frame, "faces": safe_call(DeepFace.extract_faces, {"img_path": frame, "detector_backend": detector, "enforce_detection": enforce_detection})}

def cmd_find(args_or_req) -> Any:
    """Find: search a database for similar faces from a single frame."""
    if isinstance(args_or_req, argparse.Namespace):
        img = getattr(args_or_req, "img", None)
        db = getattr(args_or_req, "db", None)
        detector = getattr(args_or_req, "detector", None)
        enforce_detection = getattr(args_or_req, "enforce_detection", False)
        model = getattr(args_or_req, "model", None)
    elif isinstance(args_or_req, dict):
        img = args_or_req.get("img")
        db = args_or_req.get("db")
        detector = args_or_req.get("detector")
        enforce_detection = args_or_req.get("enforce_detection", False)
        model = args_or_req.get("model")
    else:
        raise ValueError("Unsupported input type for cmd_find")

    if not img or not db:
        raise ValueError("find requires img and db")

    kwargs = {"img_path": img, "db_path": db, "enforce_detection": enforce_detection}
    if detector:
        kwargs["detector_backend"] = detector
    if model:
        kwargs["model_name"] = model
        kwargs["model"] = model

    return safe_call(DeepFace.find, kwargs)

def cmd_test(_args=None) -> Any:
    """Simple health-check command for the server; returns 'ok'."""
    return "ok"

# ----------------------------
# WebSocket server
# ----------------------------
async def process_and_respond(ws, req: Dict[str, Any]):
    request_id = req.get("requestId")
    cmd      = req.get("cmd")

    try:
        # --- route command ---
        if cmd == "analyze":
            res = cmd_analyze(req)
        elif cmd == "verify":
            res = cmd_verify(req)
        elif cmd == "detect":
            res = cmd_detect(req)
        elif cmd == "find":
            res = cmd_find(req)
        elif cmd == "test":
            res = cmd_test()
        else:
            raise ValueError(f"Unsupported cmd '{cmd}'")

        resp = {"requestId": request_id,
                "status": "ok",
                "command": cmd,
                "data": make_serializable(res)}

    except Exception as e:
        # NEVER let the exception reach the event-loop
        tb = traceback.format_exc()
        eprint(f"[ERROR] requestId={request_id} cmd={cmd}")
        eprint(tb)
        resp = {"requestId": request_id,
                "status": "error",
                "command": cmd,
                "data": {"message": str(e), "traceback": tb}}

    # always send something back
    try:
        await ws.send(json.dumps(resp, ensure_ascii=False))
    except Exception as send_err:
        # even the send might fail if client vanished – log and forget
        eprint(f"[WARN] failed to send response: {send_err}")


async def ws_handler(websocket):          # <-- single param
    client = f"{websocket.remote_address[0]}:{websocket.remote_address[1]}"
    logging.info("[INFO] WS client connected: %s", client)
    try:
        async for raw in websocket:
            try:
                req = json.loads(raw)
            except json.JSONDecodeError as e:
                await websocket.send(json.dumps({"status":"error","message":"Invalid JSON"}))
                continue
            await process_and_respond(websocket, req)
    except websockets.exceptions.ConnectionClosed:
        logging.info("Client disconnected: %s", client)



# ----------------------------
# CLI
# ----------------------------
def build_parser():
    p = argparse.ArgumentParser(description="DeepFace CLI / WebSocket worker")
    sub = p.add_subparsers(dest="cmd", required=True)

    # SERVE WEBSOCKET
    s = sub.add_parser("serve", help="Run WebSocket server")
    s.add_argument("--host", default="127.0.0.1")
    s.add_argument("--port", type=int, default=8765)

    # ANALYZE
    a = sub.add_parser("analyze", help="Analyze one frame")
    a.add_argument("--frame", required=True)
    a.add_argument("--actions")
    a.add_argument("--detector")
    a.add_argument("--enforce-detection", dest="enforce_detection", action="store_true")
    a.add_argument("--model")
    a.set_defaults(func=cmd_analyze)

    # VERIFY
    v = sub.add_parser("verify", help="Verify two images")
    v.add_argument("--img1", required=True)
    v.add_argument("--img2", required=True)
    v.add_argument("--detector")
    v.add_argument("--enforce-detection", dest="enforce_detection", action="store_true")
    v.add_argument("--model")
    v.set_defaults(func=cmd_verify)

    # DETECT
    d = sub.add_parser("detect", help="Detect faces in one frame")
    d.add_argument("--frame", required=True)
    d.add_argument("--detector")
    d.add_argument("--enforce-detection", dest="enforce_detection", action="store_true")
    d.set_defaults(func=cmd_detect)

    # FIND
    f = sub.add_parser("find", help="Find similar faces in DB from one frame")
    f.add_argument("--img", required=True)
    f.add_argument("--db", required=True)
    f.add_argument("--detector")
    f.add_argument("--enforce-detection", dest="enforce_detection", action="store_true")
    f.add_argument("--model")
    f.set_defaults(func=cmd_find)

    # TEST
    t = sub.add_parser("test", help="health check")
    t.set_defaults(func=cmd_test)

    return p

def main():
    parser = build_parser()
    args = parser.parse_args()

    if args.cmd == "serve":
        host = getattr(args, "host", "127.0.0.1")
        port = getattr(args, "port", 8765)
        eprint(f"[INFO] Starting WebSocket server on {host}:{port}")
        # loop = asyncio.get_event_loop()
        # loop.run_until_complete(websockets.serve(ws_handler, host, port))
        # loop.run_forever()
        # return 0

        async def serve():
            server = await websockets.serve(ws_handler, host, port)
            eprint(f"[INFO] WebSocket server started successfully on {host}:{port}")
            await server.wait_closed()

        try:
            asyncio.run(serve())
        except KeyboardInterrupt:
            eprint("[INFO] Server stopped by user")
        return 0




    try:
        result = args.func(args)
        print(json.dumps(make_serializable(result), ensure_ascii=False))
        return 0
    except Exception as e:
        eprint("ERROR:", str(e))
        traceback.print_exc(file=sys.stderr)
        print(json.dumps({"error": str(e)}))
        return 2

if __name__ == "__main__":
    raise SystemExit(main())
