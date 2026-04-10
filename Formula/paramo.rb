class Paramo < Formula
  desc "Distraction blocker for macOS with CLI and TUI"
  homepage "https://github.com/attinto/homebrew-paramo"
  url "https://github.com/attinto/homebrew-paramo.git",
      tag: "v0.1.3",
      revision: "006395bc60d5e725b549e15555d4145dc2ca2c2b"
  license "MIT"
  head "https://github.com/attinto/homebrew-paramo.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  def caveats
    <<~EOS
      Homebrew instala solo el binario CLI.

      Para activar el bloqueo del sistema y registrar el daemon:
        sudo paramo install

      Para comprobar la instalación:
        paramo doctor

      Si una actualización cambia el daemon o la configuración del sistema,
      vuelve a ejecutar:
        sudo paramo install
    EOS
  end

  test do
    output = shell_output("#{bin}/paramo config show")
    assert_match "[schedule]", output
    assert_match "[sites]", output
  end
end
