import { invoke } from "@tauri-apps/api/core";

export type PastedImage = {
  path: string;
  relativePath: string;
  preview: string;
};

function toBase64(bytes: Uint8Array) {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

function isImageFile(file: File) {
  return file.type.startsWith("image/") || /\.(png|jpe?g|webp|gif|bmp)$/i.test(file.name);
}

export function imageFilesFromTransfer(data: DataTransfer | null): File[] {
  if (!data) return [];
  const files: File[] = [];
  if (data.files?.length) {
    for (const file of Array.from(data.files)) {
      if (isImageFile(file)) files.push(file);
    }
  }
  if (files.length === 0 && data.items) {
    for (const item of Array.from(data.items)) {
      if (item.kind === "file" && item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) files.push(file);
      }
    }
  }
  return files;
}

export async function savePastedFiles(projectPath: string, files: File[]): Promise<PastedImage[]> {
  const out: PastedImage[] = [];
  for (const file of files) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const saved = await invoke<PastedImage>("save_pasted_image", {
      projectPath,
      dataBase64: toBase64(bytes),
      mime: file.type || "image/png",
    });
    out.push({
      ...saved,
      preview: URL.createObjectURL(file),
    });
  }
  return out;
}

export function imageBrief(images: { relativePath: string }[]) {
  if (images.length === 1) {
    return `Image collée : ${images[0].relativePath}`;
  }
  const list = images.map((item) => `- ${item.relativePath}`).join("\n");
  return `Images collées :\n${list}`;
}
