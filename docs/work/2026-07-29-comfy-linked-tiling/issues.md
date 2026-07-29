# Issues — comfy-linked-tiling

_Defects found during execution. Known inputs: the ComfyUI box must be reachable
(`COMFYUI_URL`, default http://172.16.10.10:8188 — generate.py's client soft-fails with a
clear message when it isn't); SDXL inpainting wants a working canvas well above 128px
cells, so layouts upscale before inpaint and downsample after (the sprite-gen pipeline's
normal mode of operation); masked inpainting guarantees outside-mask identity ONLY if the
VAE round-trip is composited back through the mask — the P2 acceptance checks it, don't
assume it._
