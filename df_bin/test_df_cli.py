# test_df.py
import asyncio, json, sys
import websockets

testImgPath1 = "C:/Users/user/PPRO_extensions/Videos_temp/angry.jpeg"
testImgPath2 = "C:/Users/user/PPRO_extensions/Videos_temp/angry2.jpeg"



async def test(cmd: str, payload: dict):
    uri = "ws://127.0.0.1:8766"
    async with websockets.connect(uri) as ws:
        req = {"requestId": 1, "cmd": cmd, **payload}
        await ws.send(json.dumps(req))
        resp = await ws.recv()
        print(json.dumps(json.loads(resp), indent=2))

if __name__ == "__main__":
    # 1) health check
    asyncio.run(test("test", {}))
    # 2) analyse one frame
    asyncio.run(test("analyze", {
        "frame": testImgPath2,
        "actions": "emotion",
        "detector": "opencv"
    }))