import { ColorType, createChart, type IChartApi, type ISeriesApi, type UTCTimestamp } from "lightweight-charts";
import { useEffect, useRef } from "react";
import { poolColor } from "../scene/pools";
import { useCity } from "../store";

export function PriceChart() {
  const host = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<Map<string, ISeriesApi<"Line">>>(new Map());
  const chart = useCity((state) => state.chart);
  const pools = useCity((state) => state.pools);
  const ids = Object.keys(pools).sort().join("|");

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const created = createChart(element, {
      width: element.clientWidth,
      height: element.clientHeight,
      layout: {
        background: { type: ColorType.Solid, color: "transparent" },
        textColor: "#9fb0c6",
        fontFamily: "IBM Plex Mono, monospace",
        fontSize: 11,
      },
      grid: {
        vertLines: { color: "rgba(255,255,255,0.05)" },
        horzLines: { color: "rgba(255,255,255,0.05)" },
      },
      rightPriceScale: { borderColor: "transparent", scaleMargins: { top: 0.1, bottom: 0.1 } },
      localization: {
        priceFormatter: (price: number) => `${price >= 0 ? "+" : ""}${price.toFixed(1)} bp`,
      },
      timeScale: { borderColor: "transparent", timeVisible: true, secondsVisible: true },
      crosshair: { vertLine: { color: "rgba(255,255,255,0.15)" }, horzLine: { color: "rgba(255,255,255,0.15)" } },
    });
    chartRef.current = created;
    const observer = new ResizeObserver(() => {
      created.applyOptions({ width: element.clientWidth, height: element.clientHeight });
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
      created.remove();
      chartRef.current = null;
      seriesRef.current.clear();
    };
  }, []);

  useEffect(() => {
    const created = chartRef.current;
    if (!created) return;
    const live = new Set(Object.keys(pools));
    for (const [id, series] of seriesRef.current) {
      if (!live.has(id)) {
        created.removeSeries(series);
        seriesRef.current.delete(id);
      }
    }
    for (const pool of Object.values(pools)) {
      if (seriesRef.current.has(pool.id)) continue;
      seriesRef.current.set(
        pool.id,
        created.addLineSeries({
          color: poolColor(pool.id),
          lineWidth: 2,
          title: "",
          priceLineVisible: false,
          lastValueVisible: false,
        }),
      );
    }
    for (const [id, series] of seriesRef.current) {
      const data = chart
        .filter((point) => point.prices[id] != null)
        .map((point) => {
          const values = Object.values(point.prices).filter((value) => Number.isFinite(value));
          const mean = values.reduce((sum, value) => sum + value, 0) / Math.max(values.length, 1);
          const price = point.prices[id] as number;
          const bp = mean > 0 ? ((price / mean) - 1) * 10000 : 0;
          return { time: point.time as UTCTimestamp, value: bp };
        });
      series.setData(data);
    }
    if (chart.length > 1) created.timeScale().fitContent();
  }, [chart, ids, pools]);

  return <div ref={host} className="chart-host" />;
}
