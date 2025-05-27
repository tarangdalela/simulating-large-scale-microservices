import CallGraphVisualizer from "@/components/CallGraphVisualizer";

export default function HomePage() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-between p-8">
      <h1 className="text-3xl font-bold mb-6">Call Graph Visualizer</h1>
      <CallGraphVisualizer />
    </main>
  );
}
