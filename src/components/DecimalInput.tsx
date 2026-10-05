import { TextInput, type TextInputProps } from "@mantine/core";

const PARTIAL = /^-?\d*\.?\d*$/;

type Props = Omit<TextInputProps, "value" | "onChange"> & {
  value: string;
  onChange: (value: string) => void;
  /** "$" shows a currency prefix, "%" a percent suffix; any other string is shown as a suffix. */
  unit?: string;
  allowNegative?: boolean;
};

/** Text-based decimal entry: keeps exactly what the person typed (no binary float rounding). */
export function DecimalInput({ value, onChange, unit, allowNegative, ...rest }: Props) {
  return (
    <TextInput
      inputMode="decimal"
      autoComplete="off"
      value={value}
      onChange={(e) => {
        const v = e.currentTarget.value.replace(/,/g, "");
        if (PARTIAL.test(v) && (allowNegative || !v.startsWith("-"))) onChange(v);
      }}
      onBlur={(e) => {
        // Normalise "", ".", "5." on blur so the backend always receives a valid decimal.
        const v = value;
        if (v === "" || v === "." || v === "-") onChange("0");
        else if (v.endsWith(".")) onChange(v.slice(0, -1));
        rest.onBlur?.(e);
      }}
      leftSection={unit === "$" ? "$" : undefined}
      leftSectionWidth={unit === "$" ? 28 : undefined}
      rightSection={unit && unit !== "$" ? unit : undefined}
      rightSectionWidth={unit && unit !== "$" ? Math.max(32, unit.length * 9 + 16) : undefined}
      styles={{ input: { fontVariantNumeric: "tabular-nums" }, section: { color: "var(--mantine-color-dimmed)", fontSize: 13 } }}
      {...rest}
    />
  );
}
