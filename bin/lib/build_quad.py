#!/usr/bin/env python3
"""Build the QUADRUPED specialist dataset from the culled animal-lora set.

Image prep is DELEGATED to prep_train.normalise() so the quadruped set gets the same ESRGAN
upscale + longer-side scale normalisation as any other set — this file previously did its own
LANCZOS resize with no normalisation, which is exactly the defect
docs/work/2026-07-25-sprite-gen-quality/ P2 exists to fix (issues I12).

Roster: mammalian quadrupeds only. Drops birds/snakes/arthropods/humanoids/mimics,
reptiles (sprawling posture), griffon (winged), pinnipeds (flipper blobs). Keeps
mammalian fantasy quadrupeds (warg, direwolf, thrumbo, megasloth, tox-*).

Layout uses kohya <repeats>_<class> folders to over-weight the weak views:
    1_quadruped_east/   (side profile  - already strong)
    2_quadruped_south/  (front view    - WEAK, 2x)
    2_quadruped_north/  (back view     - 2x)

Caption (token order stable vs run-2 so warm-start weights transfer):
    rd_style, rd_animal, rd_quadruped, rd_<dir>, <facing cues>, [family], <subject>,
    single creature, full body, white background
"""
import argparse, os, re, glob, shutil, collections, sys
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import prep_train

SRC = os.path.join(prep_train.REPO, ".staging", "animal-lora")
DST = os.path.join(prep_train.REPO, ".staging", "quad-lora-train")
SIZE = int(os.environ.get('RD_PREP_SIZE', 768))   # train-res; 1024 for SDXL-native runs

# ---- everything NOT a mammalian quadruped ----
BIPED = {"AEXP_WildGoose","Cassowary","Chicken","Crane","Duck","Emu","Goose","Megachicken",
 "Ostrich","Peacock","Penguin","Pheasant","Quail","Swan","Turkey","VAE_Moa","VAE_Rockhopper",
 "AEXP_Kangaroo"}
LEGLESS = {"AEXP_Anaconda","AEXP_Rattlesnake","Cobra","CaveCobra","ForestCobra",
 "CaveGiantConstrictor","ForestGiantConstrictor","LindwurmCave","LindwurmForest"}
MULTIPED = {"AEXP_Megascorpion","Megascarab","Megaspider","Megatardi","Spelopede",
 "Deathstinger","Redback","Webknecht","Toxscorpion"}
HUMANOID = {"AncientTroll","AncientUndead","Ghoul1","Ghoul2","Ghoul3","UnholdPlain",
 "UnholdSnow","UnholdSwamp","GolemGold","GolemIron","GolemPlasteel","GolemSilver",
 "GolemSteel","SchratDark","SchratPlain","SaplingDark","SaplingPlain"}
OTHER = {"Mimic","RoyalMimic","AEXP_Boombat","Griffon"}          # no body plan / winged
REPTILE = {"AEXP_Crocodile","AEXP_DesertTortoise","AEXP_GilaMonster","Alligator",
 "Hydra","Iguana","Pestigator","Tortoise","Toxiguana"}            # sprawling posture
PINNIPED = {"Seal","Walrus"}                                      # flippers, not legs
EXCLUDE = BIPED | LEGLESS | MULTIPED | HUMANOID | OTHER | REPTILE | PINNIPED

FAMILIES = {
 "feline": {"AEXP_CatAbyssinian","AEXP_CatBengal","AEXP_CatBritishShorthair","AEXP_CatMaineCoon",
   "AEXP_CatMunchkin","AEXP_CatNorwegian","AEXP_CatPersian","AEXP_CatSiamese","AEXP_CatSomali",
   "AEXP_CatSphynx","Cat","AEXP_Cheetah","Cougar","AEXP_Jaguar","AEXP_Lion","Lynx","Tiger",
   "RoyalTiger","Toxlion"},
 "canine": {"AEXP_Beagle","AEXP_Chihuahua","AEXP_Corgi","AEXP_FrenchBulldog","AEXP_GermanShepherd",
   "AEXP_GreatDane","AEXP_Poodle","AEXP_Pug","AEXP_Rottweiler","AEXP_ShihTzu","AEXP_WelshTerrier",
   "Husky","Labrador","YorkshireTerrier","VAE_WildDog","Wolf_Timber","Wolf_Arctic","Direwolf",
   "WhiteDirewolf","Warg","AEXP_Coyote","AEXP_ArcticCoyote","Fox_Red","Fox_Arctic","Fox_Fennec",
   "Toxafox","AEXP_Hyena","Hyena"},
 "cattle": {"Cow","Bison","Yak","Muskox","Muffalo","Wasteffalo","AngusFemale","AngusMale",
   "RavelderFemale","RavelderMale"},
 "pig": {"Pig","WildBoar","NorthernBoarFemale","NorthernBoarMale","DireBoar","Wasteboar",
   "GloucestershireFemale","GloucestershireMale","HampshireFemale","HampshireMale"},
 "deer": {"Deer","Elk","Moose","Caribou","Daer","Toxdeer"},
 "equine": {"Horse","Donkey","AEXP_Zebra","VAE_Quagga"},
 "camel": {"AEXP_Camel","Dromedary","Alpaca","Alp1","Alp2","Alp3"},
 "pachyderm": {"Elephant","AEXP_IndianElephant","Rhino","VAE_BlackRhino","Hippo","AEXP_Tapir"},
 "bear": {"Bear","AEXP_BlackBear","Toxbear","VAE_Panda"},
 "rodent": {"AEXP_AmericanBeaver","Beaver","Rat","Boomrat","Squirrel","Chinchilla","GuineaPig",
   "Capybara","Porcupine"},
 "primate": {"Gorilla","Orangutan","Monkey","AEXP_Mandrill","AEXP_Lemur","VAE_Bonobo"},
}
FAM_OF = {f: fam for fam, fs in FAMILIES.items() for f in fs}

DIR = {"east":  ("rd_east",  "side profile, side view, facing right",      "1_quadruped_east"),
       "north": ("rd_north", "back view, facing away, seen from behind",   "2_quadruped_north"),
       "south": ("rd_south", "front view, facing the viewer, facing forward", "2_quadruped_south")}
DIRRE = re.compile(r"_(east|north|south)$", re.I)

def subject(stem):
    s = DIRRE.sub("", stem)
    s = re.sub(r"^(AEXP|VAE)_", "", s).replace("_", " ")
    s = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", s)
    s = re.sub(r"(?<=[A-Za-z])(?=[0-9])", " ", s)
    return re.sub(r"\s+", " ", s).strip().lower()

_ap = argparse.ArgumentParser(prog="build_quad")
_ap.add_argument("--upscale", choices=["esrgan", "lanczos"], default="esrgan")
_ap.add_argument("--fill", type=float, default=0.85)
_ap.add_argument("--jitter", type=float, default=0.05)
_ap.add_argument("--dst", default=None)
_A = _ap.parse_args()
if _A.dst: DST = _A.dst if os.path.isabs(_A.dst) else os.path.join(prep_train.REPO, _A.dst)
FILL = _A.fill
JITTER = _A.jitter
USE_ESRGAN = _A.upscale == "esrgan"
if USE_ESRGAN:
    import urllib.request
    try: urllib.request.urlopen(prep_train.COMFY + "/system_stats", timeout=8)
    except Exception as e:
        print(f"build_quad: ComfyUI unreachable ({e}); LANCZOS"); USE_ESRGAN = False
print(f"build_quad: upscale={'esrgan' if USE_ESRGAN else 'lanczos'} fill={FILL}+/-{JITTER} -> {DST}")
if os.path.isdir(DST): shutil.rmtree(DST)
kept, dropped = set(), set()
counts = collections.Counter(); famcount = collections.Counter()
for png in sorted(glob.glob(os.path.join(SRC, "*", "*.png"))):
    folder = os.path.basename(os.path.dirname(png))
    stem = os.path.basename(png)[:-4]
    m = DIRRE.search(stem)
    if not m: continue
    if folder in EXCLUDE:
        dropped.add(folder); continue
    kept.add(folder)
    rd_dir, phrase, sub = DIR[m.group(1).lower()]
    out_dir = os.path.join(DST, sub); os.makedirs(out_dir, exist_ok=True)
    name = f"{folder}__{stem}"
    # jittered per source file — a pinned fill taught run-4 to draw the margin (I3 of
    # docs/work/2026-07-26-sprite-eval-trust/)
    _f = prep_train.jittered_fill(FILL, JITTER, os.path.basename(png))
    prep_train.normalise(Image.open(png), SIZE, _f, "esrgan", USE_ESRGAN).save(
        os.path.join(out_dir, name + ".png"))
    fam = FAM_OF.get(folder)
    toks = ["rd_style","rd_animal","rd_quadruped", rd_dir, phrase]
    if fam: toks.append(fam); famcount[fam]+=1
    toks += [subject(stem), "single creature, full body", "white background"]
    with open(os.path.join(out_dir, name+".txt"), "w") as f:
        f.write(", ".join(toks) + "\n")
    counts[sub]+=1

print(f"kept {len(kept)} quadruped folders | dropped {len(dropped)}")
print("images per subset folder:")
for k in sorted(counts): print(f"  {k:<22} {counts[k]}")
eff = counts.get("1_quadruped_east",0) + 2*counts.get("2_quadruped_south",0) + 2*counts.get("2_quadruped_north",0)
print(f"effective images/epoch (with repeats): {eff}")
print("families:", dict(famcount))
nofam = sorted(f for f in kept if f not in FAM_OF)
print(f"\nkept w/o family word ({len(nofam)}): {', '.join(nofam)}")
