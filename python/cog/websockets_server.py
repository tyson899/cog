#!/usr/bin/env python
import asyncio
import inspect
import json
import logging
import os
import signal

from websockets.asyncio.server import serve

from .predictor import get_predict, load_predictor_from_ref, run_setup
from .logging import setup_logging


def build_run(predictor_ref: str):
    predictor = load_predictor_from_ref(predictor_ref)
    if predictor is None:
        msg = "predictor is None"
        raise ValueError(msg)

    if hasattr(predictor, "setup"):
        run_setup(predictor)

    predict = get_predict(predictor)

    async def run(websocket):
        async for message in websocket:
            event = json.loads(message)
            if event["type"] == "prediction":
                await websocket.send(
                    json.dumps(
                        predict(**event["payload"])
                    )
                )
            else:
                await websocket.send(
                    json.dumps(
                        {
                            "type": "error",
                            "message": f"unknown event type {event['type']}",
                        }
                    )
                )

    return run


async def server():
    setup_logging(log_level=logging.INFO)

    loop = asyncio.get_running_loop()
    stop = loop.create_future()
    loop.add_signal_handler(signal.SIGTERM, stop.set_result, None)

    run = build_run(os.environ["COG_PREDICTOR_REF"])

    try:
        async with serve(run, "localhost", int(os.environ.get("COG_CHILD_PORT", "8765"))):
            await stop
    except asyncio.exceptions.CancelledError:
        pass


if __name__ == "__main__":
    asyncio.run(server())
