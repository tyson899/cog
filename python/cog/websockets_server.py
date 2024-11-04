#!/usr/bin/env python
import logging
import os

from starlette.websockets import WebSocket

from .predictor import get_predict, load_predictor_from_ref, run_setup
from .logging import setup_logging


def build_app(predictor_ref: str):
    predictor = load_predictor_from_ref(predictor_ref)
    if predictor is None:
        msg = "predictor is None"
        raise ValueError(msg)

    async def app(scope, receive, send):
        predict = None

        websocket = WebSocket(scope=scope, receive=receive, send=send)
        await websocket.accept()
        async for message in websocket.iter_json():
            if message["type"] == "setup":
                if hasattr(predictor, "setup"):
                    run_setup(predictor)

                predict = get_predict(predictor)

                await websocket.send_json({"type": "setup.complete"})

            elif message["type"] == "prediction":
                if predict is None:
                    await websocket.send_json(
                        {
                            "type": "error",
                            "message": "setup must be performed before predictions",
                        }
                    )
                    continue

                await websocket.send_json(predict(**message["payload"]))

            else:
                await websocket.send_json(
                    {
                        "type": "error",
                        "message": f"unknown event type {message['type']}",
                    }
                )

    return app


setup_logging(log_level=logging.INFO)

app = build_app(os.environ["COG_PREDICTOR_REF"])
