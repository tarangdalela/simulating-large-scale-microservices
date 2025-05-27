/* eslint-disable @typescript-eslint/no-explicit-any */
"use client";

import React, { useCallback, useEffect, useRef, useState } from "react";
import { Network, DataSet } from "vis-network/standalone/esm/vis-network";

interface ServiceMethodSpec {
  calls: [string[]];
}

interface ServiceSpec {
  methods: {
    [methodName: string]: ServiceMethodSpec;
  };
}

interface GraphSpec {
  services: {
    [serviceName: string]: ServiceSpec;
  };
}

export default function CallGraphVisualizer() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [graphSpec, setGraphSpec] = useState<GraphSpec | null>(null);

  const parseGraphSpec = useCallback((json: GraphSpec) => {
    const nodes = new DataSet<any>();
    const edges = new DataSet<any>();

    Object.entries(json.services).forEach(([serviceName, serviceSpec]) => {
      const serviceNodeId = `service:${serviceName}`;
      nodes.add({
        id: serviceNodeId,
        label: serviceName,
        shape: "ellipse",
        color: "#97C2FC",
      });

      Object.entries(serviceSpec.methods).forEach(
        ([methodName, methodSpec]) => {
          const methodNodeId = `method:${serviceName}.${methodName}`;
          nodes.add({
            id: methodNodeId,
            label: methodName,
            shape: "box",
            color: "#FFC107",
          });

          // Connect service to method
          edges.add({ from: serviceNodeId, to: methodNodeId });

          // Add edges for each call
          methodSpec.calls.forEach((callGroup) => {
            callGroup.forEach((call) => {
              const [calledService, calledMethod] = call.split(".");
              const targetMethodNodeId = `method:${calledService}.${calledMethod}`;
              edges.add({ from: methodNodeId, to: targetMethodNodeId });
            });
          });
        }
      );
    });

    return { nodes, edges };
  }, []);

  useEffect(() => {
    if (containerRef.current && graphSpec) {
      const { nodes, edges } = parseGraphSpec(graphSpec);
      const network = new Network(
        containerRef.current,
        { nodes, edges },
        {
          layout: { hierarchical: false },
          physics: { enabled: true }, // Enable movement
          interaction: { dragNodes: true, hover: true },
        }
      );

      return () => network.destroy();
    }
  }, [graphSpec, parseGraphSpec]);

  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (event) => {
      try {
        const json = JSON.parse(event.target?.result as string);
        setGraphSpec(json);
      } catch (error) {
        console.warn(error);
        alert("Invalid JSON");
      }
    };
    reader.readAsText(file);
  };

  return (
    <div className="flex flex-col items-center p-4">
      <input
        type="file"
        accept=".json"
        onChange={handleFileUpload}
        className="mb-4 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-blue-50 file:text-blue-700 hover:file:bg-blue-100"
      />
      <div
        ref={containerRef}
        className="w-full h-[80vh] border border-gray-300 rounded-lg"
      />
    </div>
  );
}
