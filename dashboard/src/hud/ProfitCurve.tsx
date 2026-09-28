import { Area, AreaChart, Label, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { useCity } from "../store";

export function ProfitCurve() {
  const points = useCity((state) => state.curve);
  if (points.length === 0) {
    return <p className="muted">The size curve appears when a gap beats both fees.</p>;
  }
  return (
    <ResponsiveContainer width="100%" height="100%">
      <AreaChart data={points} margin={{ top: 8, right: 12, left: 0, bottom: 14 }}>
        <XAxis dataKey="inputSol" tick={{ fill: "#8ea0b5", fontSize: 10 }} tickFormatter={(value: number) => value.toFixed(2)}>
          <Label value="size (SOL)" position="insideBottom" offset={-8} fill="#8ea0b5" fontSize={10} />
        </XAxis>
        <YAxis tick={{ fill: "#8ea0b5", fontSize: 10 }} width={52} tickFormatter={(value: number) => `${value.toFixed(3)}`} />
        <Tooltip
          contentStyle={{ background: "#101820", border: "1px solid #2a3a50", fontSize: 12 }}
          formatter={(value) => [`${Number(value).toFixed(6)} SOL`, "profit"]}
          labelFormatter={(value) => `size ${Number(value).toFixed(4)} SOL`}
        />
        <Area type="monotone" dataKey="profitSol" stroke="#7dffa8" fill="rgba(125,255,168,0.18)" strokeWidth={2} />
      </AreaChart>
    </ResponsiveContainer>
  );
}
