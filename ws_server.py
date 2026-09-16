# echo_server.py
import asyncio
import websockets

async def echo(websocket):
    async for message in websocket:
        await websocket.send(message)

async def main():
    async with websockets.serve(echo, "127.0.0.1", 8000):
        await asyncio.Future()

asyncio.run(main())
