import { NumberInput, type NumberInputProps } from "@mantine/core";
import { useEffect, useState } from "react";
import { fromDisplay, toDisplay, useDistanceUnit } from "../lib/distance";

type Props = Omit<NumberInputProps, "value" | "onChange" | "suffix" | "min" | "max"> & {
  /** Always kilometres in and out. */
  km: number | null;
  onChange: (km: number | null) => void;
  minKm?: number;
  maxKm?: number;
};

const round2 = (v: number) => Math.round(v * 100) / 100;

/** Distance entry in the person's chosen unit. Leaving the field alone or switching units never
 *  changes the stored kilometres; only a typed value does. */
export function DistanceInput({ km, onChange, minKm, maxKm, ...rest }: Props) {
  const unit = useDistanceUnit();
  const display = km == null ? "" : round2(toDisplay(km, unit));
  const [draft, setDraft] = useState<string | number>(display);
  useEffect(() => {
    // Keep what the person is typing ("3.") unless the value changed from elsewhere.
    setDraft((d) => (d !== "" && km != null && Math.abs(fromDisplay(Number(d), unit) - km) < 1e-9 ? d : display));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [km, unit]);
  return (
    <NumberInput
      decimalScale={2}
      suffix={` ${unit}`}
      min={minKm != null ? round2(toDisplay(minKm, unit)) : undefined}
      max={maxKm != null ? round2(toDisplay(maxKm, unit)) : undefined}
      value={draft}
      onChange={(v) => {
        setDraft(v);
        if (v === "") onChange(null);
        else if (!Number.isNaN(Number(v))) onChange(fromDisplay(Number(v), unit));
      }}
      {...rest}
    />
  );
}
