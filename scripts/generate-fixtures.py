"""Deterministic synthetic mechanism/stress fixtures; NOT real photo acceptance."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageCms
import hashlib, json, sys

sys.stdout.reconfigure(encoding="utf-8")
root = Path(__file__).resolve().parents[1] / "fixtures" / "100x500"
manifest_path = root / "manifest.json"
if root.exists():
    raise SystemExit("Fixture directory already exists; preserve it. Use verify-fixtures.py.")
originals = root / "assets" / "中文路径"
thumbs = root / "cache" / "thumbs"
originals.mkdir(parents=True, exist_ok=True)
thumbs.mkdir(parents=True, exist_ok=True)
icc = ImageCms.ImageCmsProfile(ImageCms.createProfile("sRGB")).tobytes()
assets = []
sizes = [(2048,2048),(3840,2160),(1200,2400),(4096,512),(512,4096),(1600,1200)]
for i in range(500):
    fmt = ["png", "jpg", "webp"][i % 3]
    width, height = sizes[i % len(sizes)]
    # Keep 30 genuine high-resolution synthetic originals; remaining still 800–2048px.
    if i >= 30:
        width, height = max(256,width//2), max(256,height//2)
    transparent = fmt == "png"
    im = Image.new("RGBA" if transparent else "RGB", (width,height),
                   (245,248,252,0) if transparent else (245,248,252))
    draw = ImageDraw.Draw(im)
    for band in range(12):
        color = ((i*37+band*19)%256,(i*61+band*29)%256,(i*17+band*43)%256)
        if transparent: color += (30+band*20,)
        x0 = band*width//12
        draw.rectangle((x0,height//8,(band+1)*width//12-1,height*7//8),fill=color)
    draw.rectangle((width//12,height//3,width*11//12,height*2//3),fill=(255,255,255,230) if transparent else (255,255,255))
    draw.text((width//10,height//2), f"PROMPT SHELF POC - SYNTHETIC {i+1:03} - {width}x{height}",fill=(20,40,80,255) if transparent else (20,40,80),font_size=max(14,min(width,height)//18))
    name = f"测试原图-{i+1:03}.{fmt}"
    path = originals / name
    options = {"icc_profile":icc} if i%10==0 else {}
    if fmt=="jpg": options.update(quality=93,subsampling=0)
    elif fmt=="webp": options.update(lossless=True)
    im.save(path, **options)
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    thumb = im.copy()
    thumb.thumbnail((540,180), Image.Resampling.LANCZOS)
    thumb_path = thumbs / f"{digest}-180-v1.png"
    thumb.save(thumb_path)
    assets.append(dict(id=f"asset-{i+1:03}", original=path.relative_to(root).as_posix(),
        thumb=thumb_path.relative_to(root).as_posix(),sha256=digest,width=width,height=height,
        format=fmt,bytes=len(data),thumb_sha256=hashlib.sha256(thumb_path.read_bytes()).hexdigest(),synthetic=True,has_icc=bool(options.get("icc_profile")),alpha=transparent))
    if (i+1)%100==0: print(f"created {i+1}/500",flush=True)
cases=[]
cursor=0
titles=["透明玻璃材质优化","布偶猫商业摄影","保构图换色","场景生成","通用高清精修"]
for i in range(100):
    count = 0 if i==99 else (6 if i<5 else 5)
    group=assets[cursor:cursor+count];cursor+=count
    prompt=f"[Prompt Shelf PoC 非敏感测试 {i+1:03}] 保持产品结构、比例及构图不变。优化透明度、色彩和细节。\n第二行：用于验证完整文本与中文标点。"
    if i==98: prompt += "\n长提示词完整性测试：请保留全部内容。"*300
    cases.append(dict(id=f"case-{i+1:03}",title=f"{titles[i%5]} · {i+1:03}",prompt=prompt,
        project=f"测试项目 {i%7+1}",tags=["合成样本",titles[i%5]],fav=i%4==0,used=100-i if i<20 else 0,
        result=group[0]["id"] if group else None,refs=[x["id"] for x in group[1:]]))
cases[5]["refs"].append(assets[0]["id"])
assert cursor==500 and len(cases)==100
manifest=dict(schema_version=1,fixture_kind="synthetic",notes=["非真实实拍；不能替代真实照片、颜色及网站接收验收", "ICC是合成sRGB嵌入样本，非校色验收", "100案例、500物理原图、501个引用，1个无图案例"],assets=assets,cases=cases)
manifest_path.write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding="utf-8")
print(f"Saved {manifest_path}; total original bytes={sum(a['bytes'] for a in assets)}")
