import time


class Predictor:
    def setup(self) -> None:
        print("Model starting setup")
        time.sleep(1)
        print("Model finished setup")

    def predict(self, num: int) -> int:
        time.sleep(1)
        return num * 2
