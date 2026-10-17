/** @summary Passive game-text formatting supported by the preview; executable HTML and resource URLs are never retained. */
const PASSIVE_TAGS = new Set([
  "B", "STRONG", "I", "EM", "U", "S", "STRIKE", "BR", "SPAN", "P", "DIV",
  "UL", "OL", "LI", "FONT", "SMALL", "BIG", "SUB", "SUP", "BLOCKQUOTE",
  "H1", "H2", "H3", "H4", "H5", "H6", "TABLE", "THEAD", "TBODY", "TFOOT", "TR", "TD", "TH", "CAPTION",
]);
/** @summary Drop active containers together with their content instead of displaying script/style source as user text. */
const ACTIVE_TAGS = new Set([
  "SCRIPT", "STYLE", "SVG", "MATH", "IFRAME", "FRAME", "FRAMESET", "OBJECT", "EMBED", "APPLET",
  "BASE", "LINK", "META", "NOSCRIPT", "TEMPLATE", "FORM", "INPUT", "BUTTON", "SELECT", "OPTION", "TEXTAREA",
  "IMG", "PICTURE", "AUDIO", "VIDEO", "SOURCE", "TRACK", "CANVAS",
]);

/** @summary Validate a literal CSS color without URL, variable, expression or arbitrary function syntax.
 * @param value Candidate literal color.
 * @returns A browser-normalized safe color or an empty string.
 */
function safeColor(value: string): string {
  const literal = value.trim();
  if (!/^(?:[a-z]+|#[0-9a-f]{3,8}|(?:rgb|rgba|hsl|hsla)\([\d.,%+\-/\sdeg]*\))$/i.test(literal)) return "";
  const probe = document.createElement("span");
  probe.style.color = literal;
  return probe.style.color;
}

/** @summary Copy only bounded passive styling; never copy raw CSS text or a resource-bearing property.
 * @param source Inert parsed source element.
 * @param target Newly authored safe output element.
 */
function copyPassiveStyle(source: HTMLElement, target: HTMLElement): void {
  const color = safeColor(source.style.color || (source.tagName === "FONT" ? source.getAttribute("color") ?? "" : ""));
  if (color) target.style.color = color;
  const weight = source.style.fontWeight.trim().toLowerCase();
  if (/^(?:normal|bold|bolder|lighter|[1-9]00)$/.test(weight)) target.style.fontWeight = weight;
  const fontStyle = source.style.fontStyle.trim().toLowerCase();
  if (/^(?:normal|italic|oblique)$/.test(fontStyle)) target.style.fontStyle = fontStyle;
  const decoration = source.style.textDecoration.trim().toLowerCase();
  if (/^(?:none|(?:underline|overline|line-through)(?:\s+(?:underline|overline|line-through))*)$/.test(decoration)) target.style.textDecoration = decoration;
  const whiteSpace = source.style.whiteSpace.trim().toLowerCase();
  if (/^(?:normal|nowrap|pre|pre-wrap|pre-line|break-spaces)$/.test(whiteSpace)) target.style.whiteSpace = whiteSpace;
}

/** @summary Preserve benign legacy rollover HTML as inert formatting, flatten unknown wrappers and drop active content.
 * @param value Game-authored text containing optional passive HTML.
 * @returns Serialized HTML containing only approved elements and validated passive attributes/styles.
 * @remarks This intentionally does not preserve arbitrary HTML, links, images, forms, event handlers, scripts or URL-bearing styling.
 */
export function formatPreviewRichText(value: unknown): string {
  const input = document.createElement("template");
  input.innerHTML = String(value ?? "");
  const output = document.createElement("div");
  /** @summary Rebuild each accepted node without attaching the parsed source DOM.
   * @param node Parsed inert node.
   * @param parent Safe output parent.
   */
  function append(node: Node, parent: Node): void {
    if (node.nodeType === Node.TEXT_NODE) { parent.appendChild(document.createTextNode(node.textContent ?? "")); return; }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    const source = node as HTMLElement;
    if (source.namespaceURI !== "http://www.w3.org/1999/xhtml" || ACTIVE_TAGS.has(source.tagName)) return;
    let destination = parent;
    if (PASSIVE_TAGS.has(source.tagName)) {
      const target = document.createElement(source.tagName.toLowerCase());
      const className = source.getAttribute("class")?.trim() ?? "";
      if (className.length <= 256 && /^(?:[a-zA-Z_][\w-]*)(?:\s+[a-zA-Z_][\w-]*)*$/.test(className)) target.className = className;
      const title = source.getAttribute("title");
      if (title !== null) target.title = title.slice(0, 4096);
      copyPassiveStyle(source, target);
      parent.appendChild(target);
      destination = target;
    }
    for (const child of [...source.childNodes]) append(child, destination);
  }
  for (const child of [...input.content.childNodes]) append(child, output);
  return output.innerHTML;
}
