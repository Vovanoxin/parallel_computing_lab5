import asyncio
import websockets


async def client(client_id, lab_id):
    async with websockets.connect("ws://localhost:3000") as websocket:
        try:
            await asyncio.sleep(1)
            print(f"Try to take {lab_id} lab from {client_id} client")
            await websocket.send(f'{{"command":"takeLab","id":{lab_id}}}')

        except:
            return

        while True:
            try:
                recv = await websocket.recv()
                print(f"MSG: {recv} from {client_id}")
            except websockets.ConnectionClosedOK:
                break


async def main():
    try:
        await asyncio.gather(
            client(0, 0),
            client(1, 0),
            client(2, 0),
            client(3, 0),
            client(4, 0),
            client(5, 0),
        )

    except:
        return

asyncio.run(main())
