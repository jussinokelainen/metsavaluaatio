METSÄVALUAATIO
==============

Asennus
-------

#### Arch linux:
```bash
git clone git@github.com:jussinokelainen/metsavaluaatio.git
cd metsavaluaatio/pkgbuild/arch
makepkg -si
```

#### macOS (homebrew):
```bash
brew tap jussinokelainen/metsavaluaatio
brew trust jussinokelainen/metsavaluaatio
brew install metsavaluaatio
```

Manuaalisesti:

```bash
git clone git@github.com:jussinokelainen/metsavaluaatio.git
cd metsavaluaatio
make
sudo make install
```

Riippuvuudet:
```bash
rust
make
cmake
```

Käyttö
------

Ennen ohjelman käyttöä, luo tiedosto ~/.config/metsavaluaatio/config.toml jossa
on henkilökohtainen MML api avain.
```toml
api_key = "12345678-90ab-cdef-ghij-klmnopqrstuv"
```

Api-avaimen lisäämisen jälkeen ohjelman käyttö:

```bash
metsaval 123-456-7890-1234 Kunta
```

Ohjelma vaatii parametreina kiinteistötunnuksen ja kunnan,
kuten yllä olevassa esimerkissä.
Tuottaa .xlsx ja .kml tiedostot siihen hakemistoon jossa se ajetaan, ja
avaa selaimen osoitteessa jossa .kml tiedostosta voi tehdä karttatason.

#### Kartan luominen:
Selaimessa klikkaa 'Create new map' -> Vasemalla olevassa ikkunassa 'Untitled layer' alapuolella klikkaa 'import' -> Upload -> Valitse ohjelman luoma .kml tiedosto

Klikkaa lisätyn tason yläpuolella 'Share' ja nimeä kartta -> valtuuta kaikki kenellä on linkki avaamaan kartta -> kopioi linkki alareunasta

Ohjelman poistaminen
--------------------

Package managerin avulla, tai manuaalisen asennuksen jälkeen:
```bash
sudo make uninstall
```
HUOM: vaatii saman PREFIX kuin asentaessa.
