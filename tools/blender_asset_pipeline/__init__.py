"""OpenSCP Blender asset pipeline package."""
try:
    import bpy  # type: ignore
except ModuleNotFoundError:
    bpy = None

if bpy is not None:
    from .addon import *  # noqa: F401,F403
