import asyncio, json
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client
from contour_config import OCS_BIN, parse_mcp_json

TIN = "E16A"  # your TIN handle

async def main():
    print("OCS_BIN =", OCS_BIN)
    async with stdio_client(StdioServerParameters(command=OCS_BIN, args=["--mcp"])) as (r, w):
        async with ClientSession(r, w) as s:
            await s.initialize()
            sess = parse_mcp_json(await s.call_tool("ocs_sessions", {}))
            print("bridge:", json.dumps(sess[0].get("bridge"), indent=1))
            sid = sess[0]["session_id"]
            res = parse_mcp_json(await s.call_tool(
                "ocs_read", {"ocs_session_id": sid, "op": "civil_surface_mesh", "parameters": {"handle": TIN}}))
            res.pop("pts", None); res.pop("tris", None)
            print("civil_surface_mesh ->", json.dumps(res, indent=1)[:800])
asyncio.run(main())