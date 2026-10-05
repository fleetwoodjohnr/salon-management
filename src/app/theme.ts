import { createTheme, type MantineColorsTuple } from "@mantine/core";

// Mulberry: a berry-violet taken from professional color-chart swatches. Primary shade is the
// deep end (#7a2e5c) in light mode so white text on filled buttons stays well above 4.5:1.
const mulberry: MantineColorsTuple = [
  "#fbf1f6",
  "#f4dce9",
  "#e9b8d1",
  "#dc90b7",
  "#cf6ca0",
  "#c0508e",
  "#a8417c",
  "#94376d",
  "#7a2e5c",
  "#5f2147",
];

// Aubergine-tinted neutrals for dark mode (index 7 = body, 6 = raised surfaces).
const dark: MantineColorsTuple = [
  "#ece7f0",
  "#cbc3d3",
  "#a89eb2",
  "#7f748a",
  "#5c5266",
  "#3e3648",
  "#2b2533",
  "#211c27",
  "#18141d",
  "#120f16",
];

export const theme = createTheme({
  primaryColor: "mulberry",
  primaryShade: { light: 8, dark: 6 },
  colors: { mulberry, dark },
  fontFamily: "'Figtree Variable', system-ui, sans-serif",
  fontFamilyMonospace: "ui-monospace, 'SFMono-Regular', monospace",
  headings: {
    fontFamily: "'Figtree Variable', system-ui, sans-serif",
    fontWeight: "650",
  },
  defaultRadius: "sm",
  radius: { xs: "3px", sm: "6px", md: "8px", lg: "12px", xl: "20px" },
  cursorType: "pointer",
  autoContrast: true,
  focusRing: "auto",
  components: {
    Button: { defaultProps: { radius: "md" } },
    Paper: { defaultProps: { radius: "lg", withBorder: true } },
    Badge: { defaultProps: { radius: "xl", variant: "light", tt: "none", fw: 600 } },
    Modal: { defaultProps: { radius: "lg", centered: true } },
    Tooltip: { defaultProps: { withArrow: true, openDelay: 300 } },
    Table: { defaultProps: { verticalSpacing: "xs", highlightOnHover: true } },
  },
});

/** Chart series colors, chosen to stay distinguishable in both schemes. */
export const chartColors = ["mulberry.6", "teal.6", "yellow.7", "indigo.5", "green.7", "pink.5", "gray.6"];
