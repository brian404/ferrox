from aiohttp import web

async def hello(request):
    return web.json_response({"message": "backend alive"})

app = web.Application()
app.router.add_get("/test", hello)

web.run_app(app, host="127.0.0.1", port=8000)
