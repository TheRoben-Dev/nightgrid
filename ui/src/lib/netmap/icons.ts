/** Line icons for map nodes, drawn in a 16×16 box (stroke only). Our own simple shapes. */
export const ICONS: Record<string, string> = {
  internet: "M8 1.5a6.5 6.5 0 1 0 0 13a6.5 6.5 0 1 0 0-13z M1.5 8h13 M8 1.5c-3 3.5-3 9.5 0 13 M8 1.5c3 3.5 3 9.5 0 13",
  router: "M2 9h12v4.5H2z M5 9V3.5 M11 9V3.5 M4.5 11.25h1 M7 11.25h1",
  tailnet: "M8 2.5L2.5 12.5h11z M8 2.5v0 M8 8.5h0",
  ap: "M3 12.5h10 M5 12.5a3 3 0 0 1 6 0 M3.5 6.5a6.5 6.5 0 0 1 9 0 M5.5 8.5a3.5 3.5 0 0 1 5 0",
  switch: "M1 5.5h14v5H1z M3 8h1 M5.5 8h1 M8 8h1 M10.5 8h1 M13 8h0",
  phone: "M5 1.5h6v13H5z M7 12.5h2",
  tablet: "M2.5 2h11v12h-11z M7 12h2",
  laptop: "M3 3h10v7H3z M1 12.5h14",
  desktop: "M1.5 2.5h13v8h-13z M6 14h4 M8 10.5V14",
  tv: "M1.5 3h13v8.5h-13z M5 14h6",
  media: "M2 4.5h12v7H2z M6.5 6.25v3.5l3-1.75z",
  speaker: "M4 1.5h8v13H4z M6 10a2 2 0 1 0 4 0a2 2 0 1 0-4 0 M8 4.5h0",
  console: "M3 5h10a2 2 0 0 1 2 2v3a2 2 0 0 1-3.5 1.3L10 10H6l-1.5 1.3A2 2 0 0 1 1 10V7a2 2 0 0 1 2-2z M4.5 7v2 M3.5 8h2 M11 8h1",
  printer: "M4 6V1.5h8V6 M2 6h12v5H2z M4 9.5h8v5H4z",
  camera: "M1.5 4.5h9v7h-9z M10.5 7l4-2v6l-4-2",
  iot: "M5.5 10.5a4 4 0 1 1 5 0v1.5h-5z M6 14.5h4",
  nas: "M3 1.5h10v13H3z M3 6h10 M3 10h10 M5 3.75h1 M5 8h1 M5 12.25h1",
  server: "M2 2h12v5H2z M2 9h12v5H2z M4 4.5h1 M4 11.5h1 M10 4.5h2 M10 11.5h2",
  vm: "M2 3h12v10H2z M2 6h12 M4 4.5h0 M6 4.5h0",
  other: "M3 3h10v10H3z M6 8h4",
};

export const icon = (k: string): string => ICONS[k] ?? ICONS.other;
