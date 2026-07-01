"""Generate VoiceInput application icon (multi-size ICO)."""
import os
from PIL import Image, ImageDraw

def create_icon(size: int) -> Image.Image:
    """Create a single-size icon with green circle + white mic."""
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    cx = size / 2
    cy = size / 2
    radius = size * 0.44  # Slightly smaller than full size for padding

    # Draw green circle
    bbox = [cx - radius, cy - radius, cx + radius, cy + radius]
    draw.ellipse(bbox, fill=(0x34, 0xC7, 0x59, 255))

    # Draw white microphone (scaled to icon size)
    scale = size / 32.0  # Base design is for 32px

    # Microphone capsule (rounded rectangle)
    mic_w = 6 * scale
    mic_h = 11 * scale
    mic_x = cx - mic_w / 2
    mic_y = cy - mic_h / 2 - 1 * scale
    mic_bbox = [mic_x, mic_y, mic_x + mic_w, mic_y + mic_h]
    draw.rounded_rectangle(mic_bbox, radius=mic_w / 2, fill=(255, 255, 255, 255))

    # Microphone stand (arc)
    arc_bbox = [cx - 7 * scale, mic_y + mic_h - 2 * scale,
                cx + 7 * scale, mic_y + mic_h + 5 * scale]
    draw.arc(arc_bbox, start=20, end=160, fill=(255, 255, 255, 255),
             width=max(1, int(2 * scale)))

    # Microphone stem
    stem_x = cx
    stem_y1 = mic_y + mic_h + 3 * scale
    stem_y2 = cy + radius * 0.55
    draw.line([(stem_x, stem_y1), (stem_x, stem_y2)],
              fill=(255, 255, 255, 255), width=max(1, int(2 * scale)))

    # Base line
    base_w = 6 * scale
    draw.line([(cx - base_w / 2, stem_y2), (cx + base_w / 2, stem_y2)],
              fill=(255, 255, 255, 255), width=max(1, int(2 * scale)))

    return img

def main():
    sizes = [16, 32, 48, 256]
    # Use the largest image as the base; Pillow downscales to each requested size.
    base = create_icon(256)

    output_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'src-tauri', 'resources')
    os.makedirs(output_dir, exist_ok=True)
    output_path = os.path.join(output_dir, 'icon.ico')

    # Save as multi-size ICO (Pillow generates each size from the base image)
    base.save(
        output_path,
        format='ICO',
        sizes=[(s, s) for s in sizes],
    )
    print(f"Icon saved to {output_path}")

if __name__ == '__main__':
    main()
