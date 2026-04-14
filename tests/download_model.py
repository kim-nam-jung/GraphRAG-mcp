import os
import urllib.request

def download_file(url, dest):
    print(f"Downloading {url} to {dest}...")
    urllib.request.urlretrieve(url, dest)
    print("Download complete.")

os.makedirs("./models/harrier", exist_ok=True)
url = "https://huggingface.co/Xenova/all-MiniLM-L6-v2/resolve/main/onnx/model_quantized.onnx"
download_file(url, "./models/harrier/model.onnx")
