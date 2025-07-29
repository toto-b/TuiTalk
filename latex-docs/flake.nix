{
  description = "Reusable LaTeX dev shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";

  outputs = { self, nixpkgs, ... } @ inputs: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
  in {
    devShells.${system}.default = pkgs.mkShell {
      packages = [
        (pkgs.texlive.combine {
          inherit (pkgs.texlive)
            scheme-small
            latexmk
            acronym
            amsmath
            babel
            biblatex
            bigfoot # or collection-latexextra
            csquotes
            enumitem
            catchfile
            svg
            transparent
            footmisc
            geometry
            glossaries
            hyperref
            listings
            microtype
            nag
            pdfpages
            pgf
            setspace
            todonotes
            wrapfig
            xstring;
          })
        pkgs.inkscape
        pkgs.zathura
        pkgs.biber
      ];

      shellHook = ''
        echo "✅ LaTeX shell ready. Use:"
        echo "latexmk -pdf -shell-escape -output-directory=build main.tex"
      '';
    };
  };
}

