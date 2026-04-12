import React, { Component, type ReactNode } from "react";
import { AlertCircle } from "lucide-react";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
  moduleName?: string;
}

interface State {
    errorMessage: string;
    hasError: boolean;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = {
      errorMessage: "",
      hasError: false,
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, errorMessage: error.message };
  }

  public componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("Uncaught error in ErrorBoundary:", error, errorInfo);
  }

  public render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }
      return (
        <div className="flex flex-col items-center justify-center p-8 bg-helios-background border border-helios-red/50 rounded-lg shadow-sm text-center w-full min-h-[300px]">
          <AlertCircle className="w-12 h-12 text-helios-red mb-4" />
          <h2 className="text-xl font-bold text-helios-ink mb-2">Something went wrong</h2>
          <p className="text-helios-text/70 mb-4 max-w-md">
            The {this.props.moduleName || "component"} encountered an unexpected error and could not be displayed.
          </p>
          <div className="text-xs text-helios-red/80 font-mono bg-helios-red/10 p-4 rounded w-full overflow-auto max-w-lg mb-6 text-left break-words max-h-32">
            {this.state.errorMessage}
          </div>
          <button
            onClick={() => window.location.reload()}
            className="px-6 py-2 bg-helios-orange hover:bg-helios-orange/80 text-helios-main font-medium rounded transition"
          >
            Reload Page
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

export const withErrorBoundary = <P extends object>(
  WrappedComponent: React.ComponentType<P>,
  moduleName?: string
) => {
  return function WithErrorBoundary(props: P) {
    return (
      <ErrorBoundary moduleName={moduleName}>
        <WrappedComponent {...props} />
      </ErrorBoundary>
    );
  };
};
