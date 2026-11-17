import { invoke } from "./invoke";
import type { ImageSource } from "./task-image-node";

/** The kinds the core keeps; anything else is refused before it is sent. */
export const IMAGE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/** An image's description as the core answers it. */
export interface TaskImage {
  id: string;
  taskId: string;
  mediaType: string;
  size: number;
  createdAt: number;
}

/** Drawn images by id, kept for the window's life: an image never changes. */
const urls = new Map<string, Promise<string>>();

/** The images of one Task's notes, through the core: sent and read as raw bytes. */
export function taskImageSource(taskId: string): ImageSource {
  return {
    url: (id) => {
      let url = urls.get(id);
      if (!url) {
        url = invoke<ArrayBuffer>("read_task_image", { id })
          .then((bytes) => URL.createObjectURL(new Blob([bytes])));
        url.catch(() => urls.delete(id));
        urls.set(id, url);
      }
      return url;
    },
    add: async (file) => {
      if (!IMAGE_TYPES.includes(file.type)) throw new Error("Images must be PNG, JPEG, GIF or WebP, up to 10 MB");
      const bytes = new Uint8Array(await file.arrayBuffer());
      const image = await invoke<TaskImage>("add_task_image", bytes, {
        headers: { "task-id": taskId, "media-type": file.type },
      });
      return image.id;
    },
  };
}
