METSÄVALUAATIO
==============

Asennus
-------

```bash
git clone git@github.com:jussinokelainen/metsavaluaatio.git
cd metsavaluaatio
sudo make install
```

Asennus jos käytössä macOS, jotta asennus ei
epäonnistu käyttöoikeuksien takia:
```bash
git clone git@github.com:jussinokelainen/metsavaluaatio.git
cd metsavaluaatio
sudo make install PREFIX=/usr/local
```

Käyttö
------
```bash
metsaval 123-456-7890-1234 Kunta
```
Ohjelma vaatii parametreina kiinteistötunnuksen ja kunnan,
kuten yllä olevassa esimerkissä.
Tuottaa .xlsx tiedoston siihen hakemistoon jossa se ajetaan.

Ohjelman poistaminen
--------------------

```bash
sudo make uninstall
```
HUOM: vaatii saman PREFIX kuin asentaessa.
