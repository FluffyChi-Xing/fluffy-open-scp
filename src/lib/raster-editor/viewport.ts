/** Convert a pointer position in the canvas wrapper to a document pixel.
 *
 * The canvas plane is translated and scaled with CSS. Keeping this conversion
 * outside the component makes every raster tool (especially flood fill) use
 * the same document coordinate system.
 */
export function viewportToPixel(
  client: { x: number; y: number },
  rect: { left: number; top: number },
  pan: { x: number; y: number },
  zoom: number,
): { x: number; y: number } {
  const scale = Math.max(0.0001, zoom);
  return {
    x: Math.floor((client.x - rect.left - pan.x) / scale),
    y: Math.floor((client.y - rect.top - pan.y) / scale),
  };
}
