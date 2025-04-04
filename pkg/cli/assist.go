package cli

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"time"

	"github.com/briandowns/spinner"
	"github.com/charmbracelet/glamour"
	"github.com/replicate/replicate-go"
	"github.com/spf13/cobra"
)

var (
	assistContextPath string
)

func newAssistCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "assist <prompt>",
		Short: "Ask the Cog assistant for help",
		Long: `Ask the Cog assistant for help.

		Passing "-" as the prompt will read from stdin until EOF.`,
		RunE: cmdAssist,
		Args: cobra.ExactArgs(1),
	}

	cmd.Flags().StringVar(&assistContextPath, "path", ".", "Path to a directory containing cog.yaml and predict.py")

	return cmd
}

func cmdAssist(cmd *cobra.Command, args []string) error {
	// will always be the first arg due to the args validation above
	prompt := args[0]

	if prompt == "-" {
		inPrompt, err := io.ReadAll(cmd.InOrStdin())
		if err != nil {
			return fmt.Errorf("could not read from stdin: %w", err)
		}
		prompt = string(inPrompt)
	}

	s := spinner.New(spinner.CharSets[9], 100*time.Millisecond)
	s.Prefix = "Assistant is assisting..."
	s.Start()

	input := replicate.PredictionInput{
		"prompt":   prompt,
		"platform": runtime.GOOS + "/" + runtime.GOARCH,
	}
	if dat, err := os.ReadFile(filepath.Join(assistContextPath, "cog.yaml")); err == nil {
		input["config_source"] = string(dat)
	}
	if dat, err := os.ReadFile(filepath.Join(assistContextPath, "predict.py")); err == nil {
		input["predict_source"] = string(dat)
	}

	fn := assistPredictProd
	if os.Getenv("LOCAL_ASSIST") != "" {
		fn = assistPredictLocal
	}

	output, err := fn(cmd.Context(), input)
	s.Stop()
	if err != nil {
		return fmt.Errorf("could not run assist: %w", err)
	}

	r, _ := glamour.NewTermRenderer(
		glamour.WithAutoStyle(),
		glamour.WithWordWrap(120),
	)

	out, err := r.Render(output)
	if err != nil {
		fmt.Println("Error rendering output:", err)

		fmt.Println("Raw output:")
		fmt.Println(output)
		return nil
	}
	fmt.Print(out)

	return nil
}

func assistPredictProd(ctx context.Context, input replicate.PredictionInput) (string, error) {
	r8, err := replicate.NewClient(replicate.WithTokenFromEnv())
	if err != nil {
		return "", fmt.Errorf("could not create replicate client: %w", err)
	}
	model := "pipelines-beta/cog-assistant"

	output, err := r8.Run(ctx, model, input, nil)
	if err != nil {
		return "", fmt.Errorf("could not run model: %w", err)
	}
	return fmt.Sprintf("%v", output), nil
}

func assistPredictLocal(ctx context.Context, input replicate.PredictionInput) (string, error) {
	token := os.Getenv("REPLICATE_API_TOKEN")
	if token == "" {
		return "", fmt.Errorf("REPLICATE_API_TOKEN is not set")
	}

	inputJSON, err := json.Marshal(input)
	if err != nil {
		return "", fmt.Errorf("could not marshal input: %w", err)
	}

	payload := map[string]any{
		"input": map[string]any{
			"inputs_json":          string(inputJSON),
			"procedure_source_url": fmt.Sprintf("http://host.docker.internal:8080/coghelp?%d", time.Now().Unix()),
			"token":                os.Getenv("REPLICATE_API_TOKEN"),
		},
	}

	var body bytes.Buffer
	if err := json.NewEncoder(&body).Encode(payload); err != nil {
		return "", fmt.Errorf("could not encode payload: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, "http://localhost:5005/predictions", &body)
	if err != nil {
		return "", fmt.Errorf("could not create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return "", fmt.Errorf("could not send request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("request failed with status %s", resp.Status)
	}

	var resMsg map[string]any

	if err := json.NewDecoder(resp.Body).Decode(&resMsg); err != nil {
		return "", fmt.Errorf("could not decode response: %w", err)
	}
	if resMsg["error"] != "" {
		return "", fmt.Errorf("error from server: %s", resMsg["error"])
	}
	if resMsg["status"] != "succeeded" {
		return "", fmt.Errorf("prediction failed with status %s", resMsg["status"])
	}
	return fmt.Sprintf("%v", resMsg["output"]), nil
}
