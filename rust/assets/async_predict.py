import asyncio


class Predictor:
    async def setup(self) -> None:
        print("Model starting setup")
        await asyncio.sleep(1)
        print("Model finished setup")

    async def predict(self, num: int) -> int:
        await asyncio.sleep(1)
        return num * 2
